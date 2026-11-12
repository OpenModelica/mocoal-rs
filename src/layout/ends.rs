// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the OSMC Public License (OSMC-PL) version 1.8 or
// the GNU AGPL version 3 license, according to the recipient's choice. See LICENSE.md.

//! The ends of the links: where a line meets a node, and on which side it leaves it.
//! Links that meet in a connector form a net.
//! Mirrors `src/ends.jl` of ModelicaComponentAutolayout.jl.

use std::collections::HashMap;

use crate::layout::diagram::{BOTTOM, Diagram, LEFT, Node, Point, RIGHT, TOP, is_horizontal};
use crate::layout::placement::find_root;
use crate::numbers::sort_perm;

/// The ends of all links. End `2k` is end `a` of link k, end `2k + 1` its end `b`. `side` is the
/// side on which the line leaves the node (1 left, 2 right, 3 bottom, 4 top). `net` numbers the
/// nets, per link (from 0; Julia's net number is one more).
#[derive(Clone, Debug)]
pub struct LinkEnds {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub side: Vec<i32>,
    pub net: Vec<usize>,
}

/// What the ends of one position on a side share: a port, or nothing (the end is alone).
/// Julia's key is `pi >= 1 ? pi : -k`; it is only compared for equality.
#[derive(Clone, Copy, PartialEq)]
enum SpreadKey {
    Port(usize),
    End(usize),
}

/// The side of the node that faces the point `q`.
pub fn facing_side(n: &Node, q: Point) -> i32 {
    let dx = q.0 - n.x;
    let dy = q.1 - n.y;
    if dx.abs() >= dy.abs() {
        return if dx < 0.0 { LEFT } else { RIGHT };
    }
    if dy < 0.0 { BOTTOM } else { TOP }
}

/// Spread the ends `run` (all on side `s` of node `n`, without a known position) along that side.
pub fn spread_ends(e: &mut LinkEnds, d: &Diagram, n: &Node, s: i32, run: &[usize]) {
    let mut keys: Vec<SpreadKey> = Vec::new();
    let mut total: Vec<f64> = Vec::new();
    let mut count: Vec<i64> = Vec::new();
    let mut key_of: Vec<usize> = Vec::new();
    for &k in run {
        let l = &d.links[k / 2];
        let (_, pi) = l.end(k % 2);
        let o = &d.nodes[l.end(1 - k % 2).0];
        let key = match pi {
            Some(p) => SpreadKey::Port(p),
            None => SpreadKey::End(k),
        };
        let at = match keys.iter().position(|&q| q == key) {
            Some(at) => at,
            None => {
                keys.push(key);
                total.push(0.0);
                count.push(0);
                keys.len() - 1
            }
        };
        total[at] += if is_horizontal(s) { o.y } else { o.x };
        count[at] += 1;
        key_of.push(at);
    }
    let m = keys.len();
    let means: Vec<f64> = (0..m).map(|q| total[q] / count[q] as f64).collect();
    let order = sort_perm(&means);
    // Ranks from 1, as in Julia: they are the numerators of `t`.
    let mut rank = vec![0_usize; m];
    for q in 0..m {
        rank[order[q]] = q + 1;
    }
    for (q, &k) in run.iter().enumerate() {
        let t = rank[key_of[q]] as f64 / (m + 1) as f64;
        if s == LEFT || s == RIGHT {
            e.x[k] = n.x + (if s == LEFT { -n.w / 2.0 } else { n.w / 2.0 });
            e.y[k] = n.y - n.h / 2.0 + n.h * t;
        } else {
            e.x[k] = n.x - n.w / 2.0 + n.w * t;
            e.y[k] = n.y + (if s == BOTTOM { -n.h / 2.0 } else { n.h / 2.0 });
        }
    }
}

/// The ends of the links of a placed diagram.
pub fn link_ends(d: &Diagram) -> LinkEnds {
    let nl = d.links.len();
    let mut e = LinkEnds {
        x: vec![0.0; 2 * nl],
        y: vec![0.0; 2 * nl],
        side: vec![0; 2 * nl],
        net: vec![0; nl],
    };
    // The ends without a known position: (node, side, end).
    let mut pending: Vec<(usize, i32, usize)> = Vec::new();
    for (li, l) in d.links.iter().enumerate() {
        for which in 0..2 {
            let (ni, pi) = l.end(which);
            let n = &d.nodes[ni];
            let o = &d.nodes[l.end(1 - which).0];
            let k = 2 * li + which;
            let facing = facing_side(n, (o.x, o.y));
            if n.is_connector && pi.is_none() {
                e.x[k] = n.x;
                e.y[k] = n.y;
                e.side[k] = facing;
            } else if pi.and_then(|p| n.ports.get(p)).is_some_and(|p| p.known) {
                let p = n.port_position(pi);
                let s = n.side_of(p);
                e.x[k] = p.0;
                e.y[k] = p.1;
                e.side[k] = if s == 0 { facing } else { s };
            } else {
                e.side[k] = facing;
                pending.push((ni, facing, k));
            }
        }
    }
    // A stable sort: the ends of a run keep the order of the links.
    pending.sort_by_key(|&(node, side, _)| (node, side));
    for run in pending.chunk_by(|p, q| (p.0, p.1) == (q.0, q.1)) {
        let ends: Vec<usize> = run.iter().map(|&(_, _, k)| k).collect();
        spread_ends(&mut e, d, &d.nodes[run[0].0], run[0].1, &ends);
    }
    // Nets: union of the (node, connector) pairs that the links join
    let mut ids: HashMap<(usize, Option<usize>), usize> = HashMap::new();
    let mut parent: Vec<usize> = Vec::new();
    let mut end_id = vec![0_usize; 2 * nl];
    for (li, l) in d.links.iter().enumerate() {
        for which in 0..2 {
            end_id[2 * li + which] = *ids.entry(l.end(which)).or_insert_with(|| {
                parent.push(parent.len());
                parent.len() - 1
            });
        }
        let ra = find_root(&mut parent, end_id[2 * li]);
        let rb = find_root(&mut parent, end_id[2 * li + 1]);
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    }
    for li in 0..nl {
        e.net[li] = find_root(&mut parent, end_id[2 * li]);
    }
    e
}

/// Remove repeated points, and points on a straight stretch between their neighbours.
pub fn simplify_path(p: &[Point]) -> Vec<Point> {
    let mut q: Vec<Point> = Vec::new();
    for &pt in p {
        let differs = match q.last() {
            None => true,
            Some(last) => (last.0 - pt.0).abs() > 1.0e-6 || (last.1 - pt.1).abs() > 1.0e-6,
        };
        if differs {
            q.push(pt);
        }
    }
    let mut out: Vec<Point> = Vec::new();
    for i in 0..q.len() {
        if i > 0
            && i + 1 < q.len()
            && let Some(&a) = out.last()
        {
            let b = q[i];
            let c = q[i + 1];
            let same_x = (a.0 - b.0).abs() < 1.0e-6 && (b.0 - c.0).abs() < 1.0e-6;
            let same_y = (a.1 - b.1).abs() < 1.0e-6 && (b.1 - c.1).abs() < 1.0e-6;
            if same_x || same_y {
                continue;
            }
        }
        out.push(q[i]);
    }
    out
}

/// A route with at most two bends between two ends, without regard to what lies between them.
pub fn simple_route(a: Point, sa: i32, b: Point, sb: i32) -> Vec<Point> {
    let ha = is_horizontal(sa);
    let hb = is_horizontal(sb);
    if ha && hb {
        let mx = ((a.0 + b.0) / 10.0).round_ties_even() * 5.0;
        return simplify_path(&[a, (mx, a.1), (mx, b.1), b]);
    } else if !ha && !hb {
        let my = ((a.1 + b.1) / 10.0).round_ties_even() * 5.0;
        return simplify_path(&[a, (a.0, my), (b.0, my), b]);
    } else if ha {
        return simplify_path(&[a, (b.0, a.1), b]);
    }
    simplify_path(&[a, (a.0, b.1), b])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::diagram::{Link, Port};

    fn node(name: &str, x: f64, y: f64) -> Node {
        let mut n = Node::new(name, "T", 20.0, 20.0);
        n.x = x;
        n.y = y;
        n.placed = true;
        n.set_free();
        n
    }

    fn port(name: &str, ix: f64, iy: f64, known: bool) -> Port {
        Port {
            name: name.to_owned(),
            ix,
            iy,
            known,
            causality: 0,
            color: String::new(),
        }
    }

    #[test]
    fn paths() {
        assert_eq!(
            simplify_path(&[
                (0.0, 0.0),
                (0.0, 0.0),
                (5.0, 0.0),
                (10.0, 0.0),
                (10.0, 4.0),
                (10.0, 8.0)
            ]),
            vec![(0.0, 0.0), (10.0, 0.0), (10.0, 8.0)]
        );
        assert_eq!(simplify_path(&[(1.0, 2.0)]), vec![(1.0, 2.0)]);
        assert!(simplify_path(&[]).is_empty());
    }

    #[test]
    fn routes() {
        // (0 + 25) / 10 = 2.5 rounds to the even 2.
        assert_eq!(
            simple_route((0.0, 0.0), RIGHT, (25.0, 10.0), LEFT),
            vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (25.0, 10.0)]
        );
        assert_eq!(
            simple_route((0.0, 0.0), TOP, (20.0, 35.0), BOTTOM),
            vec![(0.0, 0.0), (0.0, 20.0), (20.0, 20.0), (20.0, 35.0)]
        );
        assert_eq!(
            simple_route((0.0, 0.0), RIGHT, (20.0, 30.0), BOTTOM),
            vec![(0.0, 0.0), (20.0, 0.0), (20.0, 30.0)]
        );
        assert_eq!(
            simple_route((0.0, 0.0), TOP, (20.0, 30.0), LEFT),
            vec![(0.0, 0.0), (0.0, 30.0), (20.0, 30.0)]
        );
        assert_eq!(
            simple_route((0.0, 5.0), RIGHT, (30.0, 5.0), LEFT),
            vec![(0.0, 5.0), (30.0, 5.0)]
        );
    }

    #[test]
    fn sides() {
        let n = node("n", 0.0, 0.0);
        assert_eq!(facing_side(&n, (5.0, 5.0)), RIGHT);
        assert_eq!(facing_side(&n, (-5.0, 4.0)), LEFT);
        assert_eq!(facing_side(&n, (1.0, -4.0)), BOTTOM);
        assert_eq!(facing_side(&n, (1.0, 4.0)), TOP);
        assert_eq!(facing_side(&n, (0.0, 0.0)), RIGHT);
    }

    #[test]
    fn ends_and_nets() {
        let mut d = Diagram::new("M");
        let mut a = node("a", -50.0, 0.0);
        a.add_port(port("y", 110.0, 0.0, true));
        let mut b = node("b", 0.0, 0.0);
        b.add_port(port("u", -120.0, 0.0, true));
        b.add_port(port("v", 0.0, 0.0, false));
        d.nodes = vec![a, b, node("c", 50.0, 40.0), node("e", 50.0, -40.0)];
        d.links = vec![
            Link::new(0, Some(0), 1, Some(0)),
            // Two ends at the unknown port `v` of b, one at b itself: two positions on its right side.
            Link::new(1, Some(1), 2, None),
            Link::new(1, None, 3, None),
            Link::new(3, None, 1, Some(1)),
        ];
        let e = link_ends(&d);
        assert_eq!((e.x[0], e.y[0], e.side[0]), (-39.0, 0.0, RIGHT));
        assert_eq!((e.x[1], e.y[1], e.side[1]), (-12.0, 0.0, LEFT));
        // Port `v`: mean y of c and e is 0; the end without a port: -40. Ranks 2 and 1 of 2.
        assert_eq!((e.x[2], e.y[2], e.side[2]), (10.0, -10.0 + 20.0 * (2.0 / 3.0), RIGHT));
        assert_eq!((e.x[4], e.y[4], e.side[4]), (10.0, -10.0 + 20.0 * (1.0 / 3.0), RIGHT));
        assert_eq!((e.x[7], e.y[7], e.side[7]), (10.0, -10.0 + 20.0 * (2.0 / 3.0), RIGHT));
        // c and e see b to their left; each has one end per link there.
        assert_eq!((e.x[3], e.y[3], e.side[3]), (40.0, 40.0, LEFT));
        assert_eq!((e.x[5], e.y[5], e.side[5]), (40.0, -50.0 + 20.0 * (1.0 / 3.0), LEFT));
        assert_eq!((e.x[6], e.y[6], e.side[6]), (40.0, -50.0 + 20.0 * (2.0 / 3.0), LEFT));
        // Links 1 and 3 meet in b.v, links 2 and 3 in e.
        assert_eq!(e.net, vec![0, 2, 2, 2]);
    }
}
