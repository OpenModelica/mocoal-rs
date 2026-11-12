// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the OSMC Public License (OSMC-PL) version 1.8 or
// the GNU AGPL version 3 license, according to the recipient's choice. See LICENSE.md.

//! Routing of the connection lines.
//!
//! `orthogonal`: every line is the cheapest path on the routing grid (A*). A path pays for its
//! length, for bends, for crossing a line of another net, for running along one, and for passing
//! through a component. Lines of the same net are drawn to each other. Short links are routed
//! first.
//!
//! A direction is the number of the side it points to (1 left, 2 right, 3 bottom, 4 top), as in
//! Julia; the tables below are read at `direction - 1`.
//! Mirrors `src/routing.jl` of ModelicaComponentAutolayout.jl.

use crate::layout::diagram::{Diagram, Point, is_horizontal, side_vector};
use crate::layout::ends::{LinkEnds, link_ends, simple_route, simplify_path};
use crate::layout::grid::{
    Router, build_router, crossed, exit_point, foreign, heap_pop, heap_push, mark_use, net_number, reserve_exits,
};
use crate::numbers::sort_perm;
use crate::options::Options;

const DIR_X: [i32; 4] = [-1, 1, 0, 0];
const DIR_Y: [i32; 4] = [0, 0, -1, 1];
const DIR_OPPOSITE: [i32; 4] = [2, 1, 4, 3];

const BEND_COST: f32 = 20.0;
const CROSS_COST: f32 = 8.0;
const OVERLAP_COST: f32 = 40.0;
const BLOCKED_COST: f32 = 150.0;
const OWN_FACTOR: f32 = 0.75;
const MAX_EXPANSIONS: usize = 200000;
/// The grid step of the router, and the coarser one when the fine grid has too many points.
const FINE_STEP: f64 = 5.0;
const COARSE_STEP: f64 = 10.0;
/// The most points a routing grid may have.
const MAX_GRID_POINTS: usize = 500000;

fn relax(r: &mut Router, s: usize, g: f32, from: usize, h: f32) {
    if r.stamp[s - 1] != r.search || g < r.cost[s - 1] {
        r.stamp[s - 1] = r.search;
        r.cost[s - 1] = g;
        r.prev[s - 1] = from as i32;
        heap_push(r, g + h, s as i32);
    }
}

/// The cheapest path of `net` from point (is, js), leaving in direction `ds`, to point (it, jt),
/// whose node lies opposite to direction `dt`. The points of the path are put into `cells`.
/// Returns false when none was found within the limit.
#[allow(clippy::too_many_arguments)]
pub fn find_path(
    r: &mut Router,
    net: usize,
    is: usize,
    js: usize,
    ds: i32,
    it: usize,
    jt: usize,
    dt: i32,
    cells: &mut Vec<usize>,
) -> bool {
    cells.clear();
    r.heap_key.clear();
    r.heap_val.clear();
    r.search = r.search.wrapping_add(1);
    let nx = r.nx;
    let goal = 4 * nx * r.ny + 1;
    let tx = r.x[it];
    let ty = r.y[jt];
    let start = 4 * (js * nx + is) + ds as usize;
    relax(
        r,
        start,
        0.0,
        0,
        OWN_FACTOR * (((r.x[is] - tx).abs() + (r.y[js] - ty).abs()) as f32),
    );
    let mut expansions: usize = 0;
    let mut found = false;
    while let Some(top) = heap_pop(r) {
        let s = top as usize;
        if s == goal {
            found = true;
            break;
        }
        if r.closed[s - 1] == r.search {
            continue;
        }
        r.closed[s - 1] = r.search;
        expansions += 1;
        if expansions > MAX_EXPANSIONS {
            break;
        }
        let d = ((s - 1) % 4 + 1) as i32;
        let c = (s - 1) / 4;
        let i = c % nx;
        let j = c / nx;
        let g = r.cost[s - 1];
        if i == it && j == jt && d != dt {
            let turn = if d == DIR_OPPOSITE[(dt - 1) as usize] {
                0.0
            } else {
                BEND_COST
            };
            relax(r, goal, g + turn, s, 0.0);
        }
        for dn in 1..=4_i32 {
            if dn == DIR_OPPOSITE[(d - 1) as usize] {
                continue;
            }
            let i2 = i as i64 + i64::from(DIR_X[(dn - 1) as usize]);
            let j2 = j as i64 + i64::from(DIR_Y[(dn - 1) as usize]);
            if i2 < 0 || i2 >= nx as i64 || j2 < 0 || j2 >= r.ny as i64 {
                continue;
            }
            let (i2, j2) = (i2 as usize, j2 as usize);
            let horizontal = is_horizontal(dn);
            let own = if horizontal { &r.own_y[j] } else { &r.own_x[i] };
            if !own.is_empty() && !own.contains(&net) {
                continue;
            }
            let len = if horizontal {
                (r.x[i2] - r.x[i]).abs() as f32
            } else {
                (r.y[j2] - r.y[j]).abs() as f32
            };
            let at = if horizontal {
                j * nx + i.min(i2)
            } else {
                j.min(j2) * nx + i
            };
            let usage = if horizontal { r.h_use[at] } else { r.v_use[at] };
            let mut cost = len;
            if usage == net_number(net) {
                cost = OWN_FACTOR * len;
            } else if usage != 0 {
                cost += OVERLAP_COST + 10.0_f32 * len;
            }
            if crossed(r, i2, j2, !horizontal, net) {
                cost += CROSS_COST;
            }
            if r.blocked[j2 * nx + i2] {
                cost += BLOCKED_COST;
            }
            if foreign(&r.reserved, j2 * nx + i2, net) {
                cost += OVERLAP_COST;
            }
            if dn != d {
                cost += BEND_COST;
                if crossed(r, i, j, true, net) || crossed(r, i, j, false, net) {
                    cost += BEND_COST;
                }
            }
            relax(
                r,
                4 * (j2 * nx + i2) + dn as usize,
                g + cost,
                s,
                OWN_FACTOR * (((r.x[i2] - tx).abs() + (r.y[j2] - ty).abs()) as f32),
            );
        }
    }
    if !found {
        return false;
    }
    let mut s = r.prev[goal - 1] as usize;
    while s != 0 {
        let cell = (s - 1) / 4;
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
        s = r.prev[s - 1] as usize;
    }
    cells.reverse();
    true
}

/// Route link `li` on the grid, and record the pieces it uses.
pub fn route_link(r: &mut Router, d: &mut Diagram, e: &LinkEnds, li: usize, cells: &mut Vec<usize>) {
    let ka = 2 * li;
    let kb = 2 * li + 1;
    let a: Point = (e.x[ka], e.y[ka]);
    let b: Point = (e.x[kb], e.y[kb]);
    let sa = e.side[ka];
    let sb = e.side[kb];
    let (ia, ja) = exit_point(r, e, ka, &d.nodes[d.links[li].a]);
    let (ib, jb) = exit_point(r, e, kb, &d.nodes[d.links[li].b]);
    // Two connectors that face each other at less than twice the clearance: a straight line
    if sa == DIR_OPPOSITE[(sb - 1) as usize] {
        let dx = f64::from(DIR_X[(sa - 1) as usize]);
        let dy = f64::from(DIR_Y[(sa - 1) as usize]);
        let ahead = if is_horizontal(sa) {
            (b.0 - a.0) * dx
        } else {
            (b.1 - a.1) * dy
        };
        let aside = if is_horizontal(sa) {
            (b.1 - a.1).abs()
        } else {
            (b.0 - a.0).abs()
        };
        let past = if is_horizontal(sa) {
            (r.x[ia] - r.x[ib]) * dx
        } else {
            (r.y[ja] - r.y[jb]) * dy
        };
        if ahead > 0.0 && aside < 0.01 && past > 0.0 {
            d.links[li].points = vec![a, b];
            return;
        }
    }
    if !find_path(r, e.net[li], ia, ja, sa, ib, jb, sb, cells) {
        d.links[li].points = simple_route(a, sa, b, sb);
        return;
    }
    let mut pts: Vec<Point> = vec![a];
    for &c in cells.iter() {
        pts.push((r.x[c % r.nx], r.y[c / r.nx]));
    }
    pts.push(b);
    d.links[li].points = simplify_path(&pts);
    for q in 1..cells.len() {
        let lo = cells[q - 1].min(cells[q]);
        if cells[q].abs_diff(cells[q - 1]) == 1 {
            mark_use(&mut r.h_use, lo, e.net[li]);
        } else {
            mark_use(&mut r.v_use, lo, e.net[li]);
        }
    }
}

/// Give every link its line, except those that keep the one they have.
pub fn route_diagram(d: &mut Diagram, opts: &Options) {
    let mut todo = 0;
    for l in d.links.iter_mut() {
        if l.keep {
            continue;
        }
        l.points = Vec::new();
        l.smooth = false;
        todo += 1;
    }
    if opts.route == "none" || todo == 0 {
        return;
    }
    let e = link_ends(d);
    let nl = d.links.len();
    let len: Vec<f64> = (0..nl)
        .map(|k| (e.x[2 * k] - e.x[2 * k + 1]).abs() + (e.y[2 * k] - e.y[2 * k + 1]).abs())
        .collect();
    if opts.route == "straight" || opts.route == "bezier" {
        for (k, l) in d.links.iter_mut().enumerate() {
            if l.keep {
                continue;
            }
            let a: Point = (e.x[2 * k], e.y[2 * k]);
            let b: Point = (e.x[2 * k + 1], e.y[2 * k + 1]);
            if opts.route == "straight" {
                l.points = vec![a, b];
            } else {
                // Julia's `min` and `max` give NaN for a NaN.
                let t = if len[k].is_nan() {
                    f64::NAN
                } else {
                    (0.4 * len[k]).clamp(15.0, 80.0)
                };
                let va = side_vector(e.side[2 * k]);
                let vb = side_vector(e.side[2 * k + 1]);
                l.points = vec![a, (a.0 + t * va.0, a.1 + t * va.1), (b.0 + t * vb.0, b.1 + t * vb.1), b];
                l.smooth = true;
            }
        }
        return;
    }
    let mut r =
        build_router(d, &e, FINE_STEP, MAX_GRID_POINTS).or_else(|| build_router(d, &e, COARSE_STEP, MAX_GRID_POINTS));
    if let Some(r) = r.as_mut() {
        reserve_exits(r, d, &e);
    }
    let mut cells: Vec<usize> = Vec::new();
    for k in sort_perm(&len) {
        if d.links[k].keep {
            continue;
        }
        match r.as_mut() {
            None => {
                d.links[k].points = simple_route(
                    (e.x[2 * k], e.y[2 * k]),
                    e.side[2 * k],
                    (e.x[2 * k + 1], e.y[2 * k + 1]),
                    e.side[2 * k + 1],
                );
            }
            Some(r) => route_link(r, d, &e, k, &mut cells),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::diagram::{Link, Node, Port};

    /// A 20x20 `Gain` (input `u` at x = -120, output `y` at x = 110 in the ±100 icon) at (x, y).
    fn gain(name: &str, x: f64, y: f64) -> Node {
        let mut n = Node::new(name, "Gain", 20.0, 20.0);
        n.x = x;
        n.y = y;
        n.placed = true;
        n.set_free();
        for (port, ix) in [("u", -120.0), ("y", 110.0)] {
            n.add_port(Port {
                name: port.to_owned(),
                ix,
                iy: 0.0,
                known: true,
                causality: 0,
                color: String::new(),
            });
        }
        n
    }

    /// The link from the output of node `a` to the input of node `b`.
    fn link(a: usize, b: usize) -> Link {
        Link::new(a, Some(1), b, Some(0))
    }

    fn chain() -> Diagram {
        let mut d = Diagram::new("Chain");
        d.nodes = vec![gain("a", -50.0, 0.0), gain("b", 0.0, 0.0), gain("c", 50.0, 0.0)];
        d.links = vec![link(0, 1), link(1, 2)];
        d
    }

    fn with_route(route: &str) -> Options {
        Options {
            route: route.to_owned(),
            ..Options::default()
        }
    }

    #[test]
    fn chain_of_the_readme() {
        let mut d = chain();
        route_diagram(&mut d, &Options::default());
        assert_eq!(d.links[0].points, vec![(-39.0, 0.0), (-12.0, 0.0)]);
        assert_eq!(d.links[1].points, vec![(11.0, 0.0), (38.0, 0.0)]);
        assert!(!d.links[0].smooth);
    }

    #[test]
    fn straight_bezier_none() {
        let mut d = chain();
        route_diagram(&mut d, &with_route("straight"));
        assert_eq!(d.links[1].points, vec![(11.0, 0.0), (38.0, 0.0)]);
        // The ends are 27 apart: the handles have the least length, 15.
        route_diagram(&mut d, &with_route("bezier"));
        assert_eq!(
            d.links[0].points,
            vec![(-39.0, 0.0), (-24.0, 0.0), (-27.0, 0.0), (-12.0, 0.0)]
        );
        assert!(d.links[0].smooth);
        d.links[1].keep = true;
        route_diagram(&mut d, &with_route("none"));
        assert!(d.links[0].points.is_empty() && !d.links[0].smooth);
        assert_eq!(d.links[1].points.len(), 4);
        assert!(d.links[1].smooth);
    }

    #[test]
    fn one_way_up() {
        // The line leaves a at x = -30 and reaches b (input at x = -22) at x = -30: straight up.
        let mut d = Diagram::new("M");
        d.nodes = vec![gain("a", -50.0, 0.0), gain("b", -10.0, 40.0)];
        d.links = vec![link(0, 1)];
        route_diagram(&mut d, &Options::default());
        assert_eq!(
            d.links[0].points,
            vec![(-39.0, 0.0), (-30.0, 0.0), (-30.0, 40.0), (-22.0, 40.0)]
        );

        let e = link_ends(&d);
        let mut r = build_router(&d, &e, FINE_STEP, MAX_GRID_POINTS).expect("a grid");
        let mut cells = Vec::new();
        let (i, j0) = exit_point(&r, &e, 0, &d.nodes[0]);
        let (it, j1) = exit_point(&r, &e, 1, &d.nodes[1]);
        assert_eq!((r.x[i], r.y[j0], r.x[it], r.y[j1]), (-30.0, 0.0, -30.0, 40.0));
        assert!(find_path(&mut r, e.net[0], i, j0, 2, it, j1, 1, &mut cells));
        assert_eq!(cells, (j0..=j1).map(|j| j * r.nx + i).collect::<Vec<usize>>());
        // A bend at the start, 40 along the line, a bend at the end.
        assert_eq!(r.cost[4 * r.nx * r.ny], 80.0);
        route_link(&mut r, &mut d, &e, 0, &mut cells);
        let used: Vec<usize> = (0..r.v_use.len()).filter(|&at| r.v_use[at] != 0).collect();
        assert_eq!(used, (j0..j1).map(|j| j * r.nx + i).collect::<Vec<usize>>());
        assert!(r.h_use.iter().all(|&u| u == 0));
    }
}
