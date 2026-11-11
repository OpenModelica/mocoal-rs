// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The routing grid: the lines on which connection lines may run, the nodes as obstacles, and the
//! record of which net uses which piece of the grid.
//!
//! Lines lie at multiples of the step, and through every connector (a line leaves a connector
//! straight). A line through a connector may only be followed by the nets of that connector, so
//! that no other line runs into it.
//!
//! Lines, points and pieces are numbered from 0: point (i, j) is number `j * nx + i`. The states
//! of the search keep Julia's numbers, from 1: state `4 * point + direction` (direction 1 to 4),
//! the goal `4 * nx * ny + 1`; state `s` is at `s - 1` in `cost`, `prev`, `stamp` and `closed`.
//! Mirrors `src/grid.jl` of ModelicaComponentAutolayout.jl.

use std::cmp::Ordering;

use crate::diagram::{BOTTOM, Diagram, LEFT, Node, RIGHT, is_horizontal};
use crate::ends::LinkEnds;
use crate::numbers::{float_order, sort_perm};
use crate::placement::box_of;

/// The routing grid: the lines through the link ends and around the nodes, the points that are
/// blocked, and the nets that use each piece.
pub struct Router {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    /// The nets that may follow a line; empty: all.
    pub own_x: Vec<Vec<usize>>,
    pub own_y: Vec<Vec<usize>>,
    pub nx: usize,
    pub ny: usize,
    pub blocked: Vec<bool>,
    /// The net whose line leaves its connector at the point, as `net_number`; 0 none, -1 several.
    pub reserved: Vec<i32>,
    /// The net on the piece from point (i, j) to (i + 1, j), as `net_number`; 0 none, -1 several.
    pub h_use: Vec<i32>,
    /// The net on the piece from point (i, j) to (i, j + 1).
    pub v_use: Vec<i32>,
    pub cost: Vec<f32>,
    /// The state before a state; 0 none.
    pub prev: Vec<i32>,
    pub stamp: Vec<i32>,
    pub closed: Vec<i32>,
    pub heap_key: Vec<f32>,
    pub heap_val: Vec<i32>,
    pub search: i32,
}

/// The grid lines between `lo` and `hi`: multiples of `step`, and the `special` ones with their nets.
pub fn grid_lines(lo: f64, hi: f64, step: f64, special: &[f64], nets: &[usize]) -> (Vec<f64>, Vec<Vec<usize>>) {
    let order = sort_perm(special);
    let mut sc: Vec<f64> = Vec::new();
    let mut so: Vec<Vec<usize>> = Vec::new();
    for &k in &order {
        let apart = match sc.last() {
            None => true,
            Some(&last) => special[k] - last > 0.01,
        };
        if apart {
            sc.push(special[k]);
            so.push(vec![nets[k]]);
        } else if let Some(last) = so.last_mut()
            && !last.contains(&nets[k])
        {
            last.push(nets[k]);
        }
    }
    let mut lines: Vec<f64> = Vec::new();
    // A free line (Julia: `own[end] === free`) is one without nets; a special line has at least one.
    let mut own: Vec<Vec<usize>> = Vec::new();
    let mut s = 0;
    let mut u = (lo / step).ceil() * step;
    while u <= hi || s < sc.len() {
        if s < sc.len() && (u > hi || sc[s] < u + 3.0) {
            if u <= hi && (sc[s] - u).abs() < 3.0 {
                u += step;
            }
            while let (Some(&last), Some(last_own)) = (lines.last(), own.last()) {
                if !(last_own.is_empty() && sc[s] - last < 3.0) {
                    break;
                }
                lines.pop();
                own.pop();
            }
            lines.push(sc[s]);
            own.push(std::mem::take(&mut so[s]));
            s += 1;
        } else {
            let add = match (lines.last(), own.last()) {
                (Some(&last), Some(last_own)) => u - last >= 3.0 || last_own.is_empty(),
                _ => true,
            };
            if add {
                lines.push(u);
                own.push(Vec::new());
            }
            u += step;
        }
    }
    (lines, own)
}

/// The index of the line at `v` (the nearest one).
pub fn line_index(lines: &[f64], v: f64) -> usize {
    let i = search_sorted_first(lines, v);
    if i >= lines.len() {
        return lines.len().saturating_sub(1);
    }
    if i > 0 && v - lines[i - 1] < lines[i] - v {
        return i - 1;
    }
    i
}

/// The first line at or beyond `v`.
pub fn line_from(lines: &[f64], v: f64) -> usize {
    search_sorted_first(lines, v - 1.0e-6).min(lines.len().saturating_sub(1))
}

/// The last line at or before `v`.
pub fn line_up_to(lines: &[f64], v: f64) -> usize {
    search_sorted_last(lines, v + 1.0e-6).max(1) - 1
}

/// The routing grid of a placed diagram, or nothing when it would have more than `limit` points.
pub fn build_router(d: &Diagram, e: &LinkEnds, step: f64, limit: usize) -> Option<Router> {
    let ids: Vec<usize> = (0..d.nodes.len()).collect();
    let bx = box_of(d, &ids);
    let pad = 100.0_f64.min(30.0 + 2.0 * d.links.len() as f64);
    let mut sx: Vec<f64> = Vec::new();
    let mut nx: Vec<usize> = Vec::new();
    let mut sy: Vec<f64> = Vec::new();
    let mut ny: Vec<usize> = Vec::new();
    for k in 0..e.side.len() {
        if is_horizontal(e.side[k]) {
            sy.push(e.y[k]);
            ny.push(e.net[k / 2]);
        } else {
            sx.push(e.x[k]);
            nx.push(e.net[k / 2]);
        }
    }
    let (x, own_x) = grid_lines(bx.0 - pad, bx.2 + pad, step, &sx, &nx);
    let (y, own_y) = grid_lines(bx.1 - pad, bx.3 + pad, step, &sy, &ny);
    let n = x.len() * y.len();
    if n > limit || n == 0 {
        return None;
    }
    let mut r = Router {
        nx: x.len(),
        ny: y.len(),
        x,
        y,
        own_x,
        own_y,
        blocked: vec![false; n],
        reserved: vec![0; n],
        h_use: vec![0; n],
        v_use: vec![0; n],
        cost: vec![0.0; 4 * n + 1],
        prev: vec![0; 4 * n + 1],
        stamp: vec![0; 4 * n + 1],
        closed: vec![0; 4 * n + 1],
        heap_key: Vec::new(),
        heap_val: Vec::new(),
        search: 0,
    };
    for node in &d.nodes {
        // Julia's `i1:i2`, from 1 and with its end, is `i1..i2` here.
        let i1 = search_sorted_first(&r.x, node.x - node.w / 2.0 - 5.0 + 1.0e-6);
        let i2 = search_sorted_last(&r.x, node.x + node.w / 2.0 + 5.0 - 1.0e-6);
        let j1 = search_sorted_first(&r.y, node.y - node.h / 2.0 - 5.0 + 1.0e-6);
        let j2 = search_sorted_last(&r.y, node.y + node.h / 2.0 + 5.0 - 1.0e-6);
        for j in j1..j2 {
            for i in i1..i2 {
                r.blocked[j * r.nx + i] = true;
            }
        }
    }
    Some(r)
}

/// Reserve the points at which the lines leave their connectors.
pub fn reserve_exits(r: &mut Router, d: &Diagram, e: &LinkEnds) {
    for (li, l) in d.links.iter().enumerate() {
        let (i, j) = exit_point(r, e, 2 * li, &d.nodes[l.a]);
        mark_use(&mut r.reserved, j * r.nx + i, e.net[li]);
        let (i, j) = exit_point(r, e, 2 * li + 1, &d.nodes[l.b]);
        mark_use(&mut r.reserved, j * r.nx + i, e.net[li]);
    }
}

/// The grid point at which the line of end `k` (of node `n`) leaves the node: (i, j).
pub fn exit_point(r: &Router, e: &LinkEnds, k: usize, n: &Node) -> (usize, usize) {
    let s = e.side[k];
    let x = e.x[k];
    let y = e.y[k];
    if s == LEFT {
        return (line_up_to(&r.x, (n.x - n.w / 2.0).min(x) - 5.0), line_index(&r.y, y));
    }
    if s == RIGHT {
        return (line_from(&r.x, (n.x + n.w / 2.0).max(x) + 5.0), line_index(&r.y, y));
    }
    if s == BOTTOM {
        return (line_index(&r.x, x), line_up_to(&r.y, (n.y - n.h / 2.0).min(y) - 5.0));
    }
    (line_index(&r.x, x), line_from(&r.y, (n.y + n.h / 2.0).max(y) + 5.0))
}

/// Add `val` with the priority `key` to the router's heap, which gives the least key first.
// The heap positions `i`, `p`, `c` count from 1.
pub fn heap_push(r: &mut Router, key: f32, val: i32) {
    r.heap_key.push(key);
    r.heap_val.push(val);
    let mut i = r.heap_key.len();
    while i > 1 {
        let p = i / 2;
        if r.heap_key[p - 1] <= key {
            break;
        }
        r.heap_key[i - 1] = r.heap_key[p - 1];
        r.heap_val[i - 1] = r.heap_val[p - 1];
        i = p;
    }
    r.heap_key[i - 1] = key;
    r.heap_val[i - 1] = val;
}

/// The value with the least key; nothing when the heap is empty.
pub fn heap_pop(r: &mut Router) -> Option<i32> {
    let top = *r.heap_val.first()?;
    let key = r.heap_key.pop()?;
    let val = r.heap_val.pop()?;
    let n = r.heap_key.len();
    if n == 0 {
        return Some(top);
    }
    let mut i = 1;
    loop {
        let mut c = 2 * i;
        if c > n {
            break;
        }
        if c < n && r.heap_key[c] < r.heap_key[c - 1] {
            c += 1;
        }
        if r.heap_key[c - 1] >= key {
            break;
        }
        r.heap_key[i - 1] = r.heap_key[c - 1];
        r.heap_val[i - 1] = r.heap_val[c - 1];
        i = c;
    }
    r.heap_key[i - 1] = key;
    r.heap_val[i - 1] = val;
    Some(top)
}

/// Record that `net` uses a piece of the grid.
pub fn mark_use(usage: &mut [i32], at: usize, net: usize) {
    if usage[at] == 0 {
        usage[at] = net_number(net);
    } else if usage[at] != net_number(net) {
        usage[at] = -1;
    }
}

/// True when another net than `net` uses the piece.
pub fn foreign(usage: &[i32], at: usize, net: usize) -> bool {
    usage[at] != 0 && usage[at] != net_number(net)
}

/// True when a line of another net passes through point (i, j) along the given axis.
pub fn crossed(r: &Router, i: usize, j: usize, horizontal: bool, net: usize) -> bool {
    let at = j * r.nx + i;
    if horizontal {
        if i + 1 < r.nx && foreign(&r.h_use, at, net) {
            return true;
        }
        if i > 0 && foreign(&r.h_use, at - 1, net) {
            return true;
        }
    } else {
        if j + 1 < r.ny && foreign(&r.v_use, at, net) {
            return true;
        }
        if j > 0 && foreign(&r.v_use, at - r.nx, net) {
            return true;
        }
    }
    false
}

/// The number of a net in `reserved`, `h_use` and `v_use`: Julia's net number, from 1.
pub fn net_number(net: usize) -> i32 {
    net as i32 + 1
}

/// Julia's `searchsortedfirst`, less one: the index of the first line that is not before `v`,
/// the number of lines when there is none.
fn search_sorted_first(lines: &[f64], v: f64) -> usize {
    lines.partition_point(|x| float_order(x, &v) == Ordering::Less)
}

/// Julia's `searchsortedlast`: the number of lines that are not after `v`.
fn search_sorted_last(lines: &[f64], v: f64) -> usize {
    lines.partition_point(|x| float_order(x, &v) != Ordering::Greater)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{Link, Port};
    use crate::ends::link_ends;

    /// Two 20x20 nodes at (-50, 0) and (0, 0): the output at the right border of the first is
    /// connected to the input at the left border of the second.
    fn pair() -> Diagram {
        let mut d = Diagram::new("M");
        for (name, x, port, ix) in [("a", -50.0, "y", 110.0), ("b", 0.0, "u", -120.0)] {
            let mut n = Node::new(name, "Gain", 20.0, 20.0);
            n.x = x;
            n.placed = true;
            n.set_free();
            n.add_port(Port {
                name: port.to_owned(),
                ix,
                iy: 0.0,
                known: true,
                causality: 0,
                color: String::new(),
            });
            d.nodes.push(n);
        }
        d.links.push(Link::new(0, Some(0), 1, Some(0)));
        d
    }

    #[test]
    fn lines() {
        // 1.005 joins the line at 1; the multiples 0 and 5 are within 3 of a special line.
        let (lines, own) = grid_lines(-12.0, 12.0, 5.0, &[7.0, 1.0, 1.005], &[2, 0, 1]);
        assert_eq!(lines, vec![-10.0, -5.0, 1.0, 7.0, 10.0]);
        assert_eq!(own, vec![vec![], vec![], vec![0, 1], vec![2], vec![]]);
        // Special lines outside the range are kept.
        let (lines, own) = grid_lines(0.0, 5.0, 5.0, &[-8.0, 20.0, 20.0], &[3, 4, 4]);
        assert_eq!(lines, vec![-8.0, 0.0, 5.0, 20.0]);
        assert_eq!(own, vec![vec![3], vec![], vec![], vec![4]]);
        let (lines, own) = grid_lines(1.0, -1.0, 5.0, &[], &[]);
        assert!(lines.is_empty() && own.is_empty());
    }

    #[test]
    fn line_numbers() {
        let lines = [-10.0, -5.0, 1.0, 7.0, 10.0];
        assert_eq!(line_index(&lines, 0.0), 2);
        assert_eq!(line_index(&lines, -3.0), 1);
        // Halfway: the line above.
        assert_eq!(line_index(&lines, -2.0), 2);
        assert_eq!(line_index(&lines, 7.0), 3);
        assert_eq!(line_index(&lines, 20.0), 4);
        assert_eq!(line_index(&lines, -20.0), 0);
        assert_eq!(line_from(&lines, 7.0), 3);
        assert_eq!(line_from(&lines, 7.1), 4);
        assert_eq!(line_from(&lines, 11.0), 4);
        assert_eq!(line_up_to(&lines, 7.0), 3);
        assert_eq!(line_up_to(&lines, 6.9), 2);
        assert_eq!(line_up_to(&lines, -20.0), 0);
        // -0.0 is before 0.0, as for Julia's `isless`.
        assert_eq!(search_sorted_first(&[-5.0, -0.0, 5.0], 0.0), 2);
        assert_eq!(search_sorted_last(&[-5.0, 0.0, 5.0], -0.0), 1);
    }

    #[test]
    fn heap() {
        let d = pair();
        let mut r = build_router(&d, &link_ends(&d), 5.0, 500000).expect("a grid");
        for (key, val) in [(1.0, 10), (1.0, 20), (0.5, 30), (1.0, 40)] {
            heap_push(&mut r, key, val);
        }
        // Equal keys do not come out in the order they went in: the order of the Julia heap.
        let popped: Vec<Option<i32>> = (0..5).map(|_| heap_pop(&mut r)).collect();
        assert_eq!(popped, vec![Some(30), Some(40), Some(10), Some(20), None]);
    }

    #[test]
    fn router() {
        let d = pair();
        let e = link_ends(&d);
        let mut r = build_router(&d, &e, 5.0, 500000).expect("a grid");
        // x from -90 to 40, y from -40 to 40, in steps of 5.
        assert_eq!((r.nx, r.ny), (27, 17));
        assert_eq!((r.x[0], r.x[26], r.y[0], r.y[16]), (-90.0, 40.0, -40.0, 40.0));
        assert_eq!(r.own_y[8], vec![0]);
        assert!(r.own_x.iter().all(|o| o.is_empty()));
        assert_eq!(r.own_y.iter().filter(|o| o.is_empty()).count(), 16);
        // Node a covers x from -60 to -40 and y from -10 to 10.
        let blocked: Vec<usize> = (0..r.nx).filter(|&i| r.blocked[8 * r.nx + i]).collect();
        assert_eq!(blocked, vec![6, 7, 8, 9, 10, 16, 17, 18, 19, 20]);
        let blocked: Vec<usize> = (0..r.ny).filter(|&j| r.blocked[j * r.nx + 6]).collect();
        assert_eq!(blocked, vec![6, 7, 8, 9, 10]);
        // The line leaves a at x = -30 (first line from -39 + 5) and b at x = -20 (last line up to -12 - 5).
        assert_eq!(exit_point(&r, &e, 0, &d.nodes[0]), (12, 8));
        assert_eq!(exit_point(&r, &e, 1, &d.nodes[1]), (14, 8));
        reserve_exits(&mut r, &d, &e);
        let reserved: Vec<usize> = (0..r.reserved.len()).filter(|&at| r.reserved[at] != 0).collect();
        assert_eq!(reserved, vec![8 * 27 + 12, 8 * 27 + 14]);
        assert_eq!(r.reserved[8 * 27 + 12], 1);
        assert!(build_router(&d, &e, 5.0, 27 * 17 - 1).is_none());
    }

    #[test]
    fn uses() {
        let mut usage = vec![0; 3];
        mark_use(&mut usage, 1, 0);
        assert_eq!(usage, vec![0, 1, 0]);
        assert!(!foreign(&usage, 1, 0) && foreign(&usage, 1, 1) && !foreign(&usage, 0, 1));
        mark_use(&mut usage, 1, 0);
        assert_eq!(usage, vec![0, 1, 0]);
        mark_use(&mut usage, 1, 1);
        assert_eq!(usage, vec![0, -1, 0]);
        assert!(foreign(&usage, 1, 0) && foreign(&usage, 1, 1));

        let d = pair();
        let mut r = build_router(&d, &link_ends(&d), 5.0, 500000).expect("a grid");
        // Net 5 runs from point (0, 1) to (1, 1), and from (26, 0) to (26, 1).
        mark_use(&mut r.h_use, r.nx, 5);
        mark_use(&mut r.v_use, 26, 5);
        assert!(crossed(&r, 0, 1, true, 0) && crossed(&r, 1, 1, true, 0));
        assert!(!crossed(&r, 2, 1, true, 0) && !crossed(&r, 1, 1, true, 5) && !crossed(&r, 1, 1, false, 0));
        assert!(crossed(&r, 26, 0, false, 0) && crossed(&r, 26, 1, false, 0) && !crossed(&r, 26, 2, false, 0));
        assert!(!crossed(&r, 26, 1, true, 0));
    }
}
