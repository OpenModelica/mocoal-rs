// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Spring placement of one connected group of nodes.
//!
//! Adapted from `spring_layout` in GraphPlot.jl (https://github.com/JuliaGraphs/GraphPlot.jl),
//! MIT "Expat" License, Copyright (c) 2015: afternone. Here a list of neighbours replaces the
//! sparse adjacency matrix, and a fixed generator replaces the random numbers, so that the same
//! model always gets the same layout.
//! Mirrors `src/spring.jl` of ModelicaComponentAutolayout.jl.

use std::cmp::Ordering;

use crate::diagram::{Diagram, Point};
use crate::numbers::float_order;
use crate::options::Options;

/// The next state of a 64-bit xorshift generator.
fn xorshift(s: u64) -> u64 {
    let mut x = s;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// Force-directed (Fruchterman-Reingold) layout of `n` vertices. `adj[i]` holds the neighbours of
/// vertex i. Returns the x and y coordinates, scaled to [-1, 1].
fn spring_layout(n: usize, adj: &[Vec<usize>], c: f64, max_iter: usize, init_temp: f64) -> (Vec<f64>, Vec<f64>) {
    let mut x: Vec<f64> = vec![0.0; n];
    let mut y: Vec<f64> = vec![0.0; n];
    let mut state: u64 = 0x9e3779b97f4a7c15;
    for i in 0..n {
        state = xorshift(state);
        x[i] = 2.0 * ((state >> 11) as f64 / 9007199254740992.0) - 1.0;
        state = xorshift(state);
        y[i] = 2.0 * ((state >> 11) as f64 / 9007199254740992.0) - 1.0;
    }
    // The optimal distance between vertices.
    let k = c * (4.0 / n as f64).sqrt();
    let k2 = k * k;
    let mut fx: Vec<f64> = vec![0.0; n];
    let mut fy: Vec<f64> = vec![0.0; n];
    let mut is_adj: Vec<bool> = vec![false; n];
    for iter in 1..=max_iter {
        for i in 0..n {
            for &j in &adj[i] {
                is_adj[j] = true;
            }
            let mut sx = 0.0_f64;
            let mut sy = 0.0_f64;
            for j in 0..n {
                if i == j {
                    continue;
                }
                let dx = x[j] - x[i];
                let dy = y[j] - y[i];
                let dist2 = (dx * dx + dy * dy).max(1.0e-12);
                let dist = dist2.sqrt();
                // Attractive and repulsive between neighbours, repulsive only otherwise.
                let f = if is_adj[j] { dist / k - k2 / dist2 } else { -k2 / dist2 };
                sx += f * dx;
                sy += f * dy;
            }
            for &j in &adj[i] {
                is_adj[j] = false;
            }
            fx[i] = sx;
            fy[i] = sy;
        }
        // Cool down, and limit the moves to the temperature.
        let temp = init_temp / iter as f64;
        for i in 0..n {
            let mag = (fx[i] * fx[i] + fy[i] * fy[i]).sqrt();
            if mag == 0.0 {
                continue;
            }
            let scale = mag.min(temp) / mag;
            x[i] += fx[i] * scale;
            y[i] += fy[i] * scale;
        }
    }
    if n == 0 {
        return (x, y);
    }
    // Julia's `minimum` and `maximum`: of two zeros the negative one is the smaller.
    let (mut x1, mut x2, mut y1, mut y2) = (x[0], x[0], y[0], y[0]);
    for i in 1..n {
        if float_order(&x[i], &x1) == Ordering::Less {
            x1 = x[i];
        }
        if float_order(&x[i], &x2) == Ordering::Greater {
            x2 = x[i];
        }
        if float_order(&y[i], &y1) == Ordering::Less {
            y1 = y[i];
        }
        if float_order(&y[i], &y2) == Ordering::Greater {
            y2 = y[i];
        }
    }
    for i in 0..n {
        x[i] = if x2 > x1 {
            2.0 * (x[i] - x1) / (x2 - x1) - 1.0
        } else {
            0.0
        };
        y[i] = if y2 > y1 {
            2.0 * (y[i] - y1) / (y2 - y1) - 1.0
        } else {
            0.0
        };
    }
    (x, y)
}

/// Place the nodes `members`, connected by the links `link_ids`, with the spring layout.
pub fn spring_component(d: &mut Diagram, members: &[usize], link_ids: &[usize], opts: &Options) {
    let n = members.len();
    let mut at: Vec<Option<usize>> = vec![None; d.nodes.len()];
    for (k, &m) in members.iter().enumerate() {
        at[m] = Some(k);
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &li in link_ids {
        let l = &d.links[li];
        let (Some(a), Some(b)) = (at[l.a], at[l.b]) else {
            continue;
        };
        if a == b {
            continue;
        }
        if !adj[a].contains(&b) {
            adj[a].push(b);
        }
        if !adj[b].contains(&a) {
            adj[b].push(a);
        }
    }
    let (xs, ys) = spring_layout(n, &adj, 2.0, 100, 2.0);
    let mut w = 0.0_f64;
    let mut h = 0.0_f64;
    for &m in members {
        w = w.max(d.nodes[m].w);
        h = h.max(d.nodes[m].h);
    }
    let half = 0.5 * (n as f64).sqrt() * 1.2;
    let mut done: Vec<usize> = Vec::new();
    for (k, &m) in members.iter().enumerate() {
        let c: Point = (xs[k] * half * (w + opts.dx), ys[k] * half * (h + opts.dy));
        d.place_near_among(m, c, opts.dx.min(opts.dy) / 2.0, &done);
        d.nodes[m].placed = false;
        done.push(m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{Link, Node};

    #[test]
    fn generator() {
        // 1 -> 0x2001 -> 0x2041 -> 0x40822041.
        assert_eq!(xorshift(1), 0x4082_2041);
        assert_eq!(xorshift(0), 0);
        assert_eq!(xorshift(0x9e37_79b9_7f4a_7c15), 0xdc1b_77ae_0bf3_4dad);
        assert_eq!(xorshift(0xdc1b_77ae_0bf3_4dad), 0x64f0_eeb9_026e_6076);
        assert_eq!(xorshift(0x64f0_eeb9_026e_6076), 0x7b07_ce91_e590_6136);
    }

    #[test]
    fn start_positions() {
        // No iterations: the start positions, x from the states 1 and 3, scaled to [-1, 1].
        // 0.719588241561633 and -0.03882425190101646 are the ends; y likewise.
        let (x, y) = spring_layout(2, &[vec![1], vec![0]], 2.0, 0, 2.0);
        assert_eq!(x, vec![1.0, -1.0]);
        assert_eq!(y, vec![1.0, -1.0]);
    }

    #[test]
    fn single_vertex() {
        let (x, y) = spring_layout(1, &[vec![]], 2.0, 100, 2.0);
        assert_eq!((x, y), (vec![0.0], vec![0.0]));
        let (x, y) = spring_layout(0, &[], 2.0, 100, 2.0);
        assert!(x.is_empty() && y.is_empty());
    }

    #[test]
    fn two_vertices_stay_on_their_sides() {
        // The forces act along the line between the two: the order on both axes is kept.
        let (x, y) = spring_layout(2, &[vec![1], vec![0]], 2.0, 100, 2.0);
        assert_eq!(x, vec![1.0, -1.0]);
        assert_eq!(y, vec![1.0, -1.0]);
    }

    #[test]
    fn pair_is_placed_on_the_grid() {
        let mut d = Diagram::new("M");
        d.nodes.push(Node::new("a", "A", 20.0, 20.0));
        d.nodes.push(Node::new("b", "A", 20.0, 20.0));
        d.links.push(Link {
            a: 0,
            pa: None,
            b: 1,
            pb: None,
            points: Vec::new(),
            eq_tok: None,
            semi_tok: None,
            ann_tok: None,
            has_line: false,
            color: String::new(),
            smooth: false,
            keep: false,
        });
        spring_component(&mut d, &[0, 1], &[0], &Options::default());
        // half = 0.5 * sqrt(2) * 1.2 = 0.848...; the corners are at +-0.848 * 50 and +-0.848 * 40,
        // rounded to the grid of 10.
        assert_eq!((d.nodes[0].x, d.nodes[0].y), (40.0, 30.0));
        assert_eq!((d.nodes[1].x, d.nodes[1].y), (-40.0, -30.0));
        assert!(!d.nodes[0].placed && !d.nodes[1].placed);
    }
}
