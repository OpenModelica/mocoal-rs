// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Placement of the nodes of a diagram.
//!
//! Nodes that are connected form a group. Every group is laid out on its own (layered or spring),
//! the groups are packed into rows, and the nodes without connections go into a grid below.
//! In keep mode, nodes with a placement stay where they are and the others are added near the
//! nodes they connect to.
//! Mirrors `src/placement.jl` of ModelicaComponentAutolayout.jl.

use crate::diagram::{ACAUSAL, Diagram, Point, overlaps, side_vector};
use crate::layered::layer_component;
use crate::options::Options;
use crate::spring::spring_component;

/// The root of `i0` in the union-find forest `parent`; the path to it is halved.
pub fn find_root(parent: &mut [usize], i0: usize) -> usize {
    let mut i = i0;
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Move the nodes `ids` by (dx, dy).
fn shift_nodes(d: &mut Diagram, ids: &[usize], dx: f64, dy: f64) {
    for &i in ids {
        d.nodes[i].x += dx;
        d.nodes[i].y += dy;
    }
}

/// The bounding box of the nodes `ids`: (x1, y1, x2, y2).
pub fn box_of(d: &Diagram, ids: &[usize]) -> (f64, f64, f64, f64) {
    let (mut x1, mut y1, mut x2, mut y2) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &i in ids {
        let n = &d.nodes[i];
        x1 = x1.min(n.x - n.w / 2.0);
        x2 = x2.max(n.x + n.w / 2.0);
        y1 = y1.min(n.y - n.h / 2.0);
        y2 = y2.max(n.y + n.h / 2.0);
    }
    if !x1.is_finite() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (x1, y1, x2, y2)
}

/// Put the nodes `ids` into a grid with `columns` columns, in their order. The top left corner of
/// the grid is (x0, y0).
fn grid_place(d: &mut Diagram, ids: &[usize], columns: usize, x0: f64, y0: f64, opts: &Options) {
    let mut w = 0.0_f64;
    let mut h = 0.0_f64;
    for &i in ids {
        w = w.max(d.nodes[i].w);
        h = h.max(d.nodes[i].h);
    }
    let px = ((w + opts.dx) / 10.0).ceil() * 10.0;
    let py = ((h + opts.dy) / 10.0).ceil() * 10.0;
    for (k, &i) in ids.iter().enumerate() {
        let n = &mut d.nodes[i];
        n.x = x0 + w / 2.0 + px * (k % columns) as f64;
        n.y = y0 - h / 2.0 - py * (k / columns) as f64;
    }
}

/// Lay out all nodes that are not fixed.
fn layout_free(d: &mut Diagram, opts: &Options) {
    let n = d.nodes.len();
    let free: Vec<usize> = (0..n).filter(|&i| !d.nodes[i].fixed).collect();
    if free.is_empty() {
        return;
    }
    if opts.layout == "grid" {
        let columns = (free.len() as f64).sqrt().ceil() as usize;
        grid_place(d, &free, columns, 0.0, 0.0, opts);
    } else {
        let mut parent: Vec<usize> = (0..n).collect();
        let mut inc: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut used: Vec<usize> = Vec::new();
        for (li, l) in d.links.iter().enumerate() {
            if l.a == l.b || d.nodes[l.a].fixed || d.nodes[l.b].fixed {
                continue;
            }
            inc[l.a].push(li);
            inc[l.b].push(li);
            used.push(li);
            let ra = find_root(&mut parent, l.a);
            let rb = find_root(&mut parent, l.b);
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
        let mut group_at: Vec<Option<usize>> = vec![None; n];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut single: Vec<usize> = Vec::new();
        for &i in &free {
            if inc[i].is_empty() {
                single.push(i);
                continue;
            }
            let r = find_root(&mut parent, i);
            let g = match group_at[r] {
                Some(g) => g,
                None => {
                    groups.push(Vec::new());
                    group_at[r] = Some(groups.len() - 1);
                    groups.len() - 1
                }
            };
            groups[g].push(i);
        }
        let mut group_links: Vec<Vec<usize>> = vec![Vec::new(); groups.len()];
        for &li in &used {
            if let Some(g) = group_at[find_root(&mut parent, d.links[li].a)] {
                group_links[g].push(li);
            }
        }
        // Largest group first, equal sizes in their order.
        let mut order: Vec<usize> = (0..groups.len()).collect();
        order.sort_by(|&a, &b| groups[b].len().cmp(&groups[a].len()));
        let mut boxes: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(groups.len());
        let gap = opts.dx.max(opts.dy) + 20.0;
        let mut area = 0.0_f64;
        let mut widest = 0.0_f64;
        for (group, links) in groups.iter().zip(&group_links) {
            if opts.layout == "spring" {
                spring_component(d, group, links, opts);
            } else {
                layer_component(d, group, links, &inc, opts);
            }
            let b = box_of(d, group);
            boxes.push(b);
            area += (b.2 - b.0 + gap) * (b.3 - b.1 + gap);
            widest = widest.max(b.2 - b.0);
        }
        let limit = widest.max((1.6 * area).sqrt());
        let mut x = 0.0_f64;
        let mut y = 0.0_f64;
        let mut row_height = 0.0_f64;
        for &g in &order {
            let bw = boxes[g].2 - boxes[g].0;
            let bh = boxes[g].3 - boxes[g].1;
            if x > 0.0 && x + bw > limit {
                y -= row_height + gap;
                x = 0.0;
                row_height = 0.0;
            }
            shift_nodes(
                d,
                &groups[g],
                ((x - boxes[g].0) / 10.0).round_ties_even() * 10.0,
                ((y - boxes[g].3) / 10.0).round_ties_even() * 10.0,
            );
            x += bw + gap;
            row_height = row_height.max(bh);
        }
        if !single.is_empty() {
            let mut w = 0.0_f64;
            for &i in &single {
                w = w.max(d.nodes[i].w);
            }
            let mut columns = (single.len() as f64).sqrt().ceil() as i64;
            if !groups.is_empty() {
                y -= row_height + gap;
                columns = columns.max(((limit + opts.dx) / (w + opts.dx)).floor() as i64);
            }
            let columns = columns.min(single.len() as i64) as usize;
            grid_place(d, &single, columns, 0.0, (y / 10.0).round_ties_even() * 10.0, opts);
        }
    }
    let b = box_of(d, &free);
    shift_nodes(
        d,
        &free,
        -((b.0 + b.2) / 20.0).round_ties_even() * 10.0,
        -((b.1 + b.3) / 20.0).round_ties_even() * 10.0,
    );
    for &i in &free {
        d.nodes[i].placed = true;
        d.nodes[i].set_free();
    }
}

/// Move the free nodes that overlap a fixed node to the nearest free position.
fn clear_fixed(d: &mut Diagram, opts: &Options) {
    let gap = opts.dx.min(opts.dy) / 2.0;
    for i in 0..d.nodes.len() {
        if d.nodes[i].fixed {
            continue;
        }
        for j in 0..d.nodes.len() {
            if d.nodes[j].fixed && overlaps(&d.nodes[i], &d.nodes[j], gap) {
                d.nodes[i].placed = false;
                let c = (d.nodes[i].x, d.nodes[i].y);
                d.place_near(i, c, gap);
                break;
            }
        }
    }
}

/// Where node `i` should go, given the placed nodes it connects to: next to their connectors.
/// Returns none when it connects to no placed node.
fn wanted_position(d: &Diagram, i: usize, inc: &[Vec<usize>], opts: &Options) -> Option<Point> {
    let n = &d.nodes[i];
    let mut sx = 0.0_f64;
    let mut sy = 0.0_f64;
    let mut count = 0_i64;
    for &li in &inc[i] {
        let l = &d.links[li];
        let (o, po, pm) = if l.a == i { (l.b, l.pb, l.pa) } else { (l.a, l.pa, l.pb) };
        if o == i || !d.nodes[o].placed {
            continue;
        }
        let other = &d.nodes[o];
        let q = other.port_position(po);
        let side = if other.is_connector { 0 } else { other.side_of(q) };
        let v = side_vector(side);
        let mine = n.port_offset(pm);
        if side == 0 {
            sx += other.x;
            sy += other.y - (other.h + n.h) / 2.0 - opts.dy;
        } else {
            sx += q.0 + v.0 * (opts.dx + n.w / 2.0) - (if v.0 == 0.0 { mine.0 } else { 0.0 });
            sy += q.1 + v.1 * (opts.dy + n.h / 2.0) - (if v.1 == 0.0 { mine.1 } else { 0.0 });
        }
        count += 1;
    }
    if count == 0 {
        return None;
    }
    Some((sx / count as f64, sy / count as f64))
}

/// Keep the fixed nodes, and add the others one by one near the nodes they connect to.
fn place_incremental(d: &mut Diagram, opts: &Options) {
    let n = d.nodes.len();
    let mut inc: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (li, l) in d.links.iter().enumerate() {
        if l.a == l.b {
            continue;
        }
        inc[l.a].push(li);
        inc[l.b].push(li);
    }
    let gap = opts.dx.min(opts.dy) / 2.0;
    let mut left = d.nodes.iter().filter(|o| !o.placed).count();
    let mut spare = 0_i64;
    while left > 0 {
        let mut wanted: Option<(usize, Point)> = None;
        for i in 0..n {
            if d.nodes[i].placed {
                continue;
            }
            if let Some(c) = wanted_position(d, i, &inc, opts) {
                wanted = Some((i, c));
                break;
            }
        }
        let (chosen, c) = match wanted {
            Some(found) => found,
            None => {
                let Some(chosen) = (0..n).find(|&i| !d.nodes[i].placed) else {
                    break;
                };
                let mut x2 = f64::NEG_INFINITY;
                let mut y2 = f64::NEG_INFINITY;
                for o in &d.nodes {
                    if o.fixed {
                        x2 = x2.max(o.x + o.w / 2.0);
                        y2 = y2.max(o.y + o.h / 2.0);
                    }
                }
                let node = &d.nodes[chosen];
                let c = (
                    x2 + 2.0 * opts.dx + node.w / 2.0,
                    y2 - node.h / 2.0 - spare as f64 * (node.h + opts.dy),
                );
                spare += 1;
                (chosen, c)
            }
        };
        d.place_near(chosen, c, gap);
        left -= 1;
    }
}

/// Julia's `sign`: a zero keeps its sign, where `f64::signum` gives 1 or -1.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

/// Turn the components whose connectors are all acausal (two-pin electrical, mechanical or thermal
/// parts) so that their connectors face the nodes they connect to. A node is turned by 0, 90, 180
/// or 270 degrees, whichever makes the lines from its connectors to the centres of its neighbours
/// shortest and leaves no connector facing away from its neighbour. Returns true when a node was
/// turned (or turned back).
fn choose_rotations(d: &mut Diagram) -> bool {
    let n = d.nodes.len();
    let mut used: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut other: Vec<Vec<usize>> = vec![Vec::new(); n];
    for l in &d.links {
        if l.a == l.b {
            continue;
        }
        if let Some(pa) = l.pa {
            used[l.a].push(pa);
            other[l.a].push(l.b);
        }
        if let Some(pb) = l.pb {
            used[l.b].push(pb);
            other[l.b].push(l.a);
        }
    }
    let mut turned = false;
    for i in 0..n {
        let nd = &d.nodes[i];
        if nd.fixed || nd.is_connector || nd.inherited || used[i].is_empty() {
            continue;
        }
        let mut ok = true;
        let mut distinct: Vec<usize> = Vec::new();
        for &pi in &used[i] {
            if !nd.ports.get(pi).is_some_and(|p| p.known && p.causality == ACAUSAL) {
                ok = false;
            }
            if !distinct.contains(&pi) {
                distinct.push(pi);
            }
        }
        if !(ok && distinct.len() >= 2) {
            continue;
        }
        let mut best = 0_i32;
        let mut best_cost = f64::INFINITY;
        let mut cost0 = 0.0_f64;
        for r in [0, 90, 180, 270] {
            let mut c = if r == nd.rot { 0.0 } else { 1.0 };
            for (&pi, &oi) in used[i].iter().zip(&other[i]) {
                let o = nd.port_offset_at(Some(pi), r);
                let t = &d.nodes[oi];
                let dx = t.x - nd.x;
                let dy = t.y - nd.y;
                c += (t.x - (nd.x + o.0)).abs() + (t.y - (nd.y + o.1)).abs();
                let ex = if o.0.abs() >= o.1.abs() { sign(o.0) } else { 0.0 };
                let ey = if o.0.abs() >= o.1.abs() { 0.0 } else { sign(o.1) };
                if ex * dx + ey * dy < 0.0 {
                    c += 20.0;
                }
            }
            if r == nd.rot {
                cost0 = c;
            }
            if c < best_cost {
                best_cost = c;
                best = r;
            }
        }
        if best != nd.rot && best_cost < cost0 - 8.0 {
            d.nodes[i].set_rotation(best);
            turned = true;
        }
    }
    turned
}

/// Place the nodes of the diagram.
pub fn place_diagram(d: &mut Diagram, opts: &Options) {
    let fixed = d.nodes.iter().filter(|n| n.fixed).count();
    if fixed == d.nodes.len() {
        return;
    }
    if opts.keep && fixed > 0 {
        place_incremental(d, opts);
    } else {
        layout_free(d, opts);
        // Turn two-pin parts along the flow, then place again with the turned boxes.
        if opts.rotate && fixed == 0 {
            for _ in 0..3 {
                if !choose_rotations(d) {
                    break;
                }
                layout_free(d, opts);
            }
        }
        if fixed > 0 {
            clear_fixed(d, opts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{Link, Node, Port};

    fn link(a: usize, pa: Option<usize>, b: usize, pb: Option<usize>) -> Link {
        Link {
            a,
            pa,
            b,
            pb,
            points: Vec::new(),
            eq_tok: None,
            semi_tok: None,
            ann_tok: None,
            has_line: false,
            color: String::new(),
            smooth: false,
            keep: false,
        }
    }

    fn port(name: &str, ix: f64, iy: f64) -> Port {
        Port {
            name: name.to_owned(),
            ix,
            iy,
            known: true,
            causality: ACAUSAL,
            color: String::new(),
        }
    }

    fn origins(d: &Diagram) -> Vec<Point> {
        d.nodes.iter().map(|n| (n.ox, n.oy)).collect()
    }

    #[test]
    fn roots() {
        let mut parent = vec![0, 0, 1, 2, 4];
        assert_eq!(find_root(&mut parent, 3), 0);
        assert_eq!(parent, vec![0, 0, 1, 1, 4]);
        assert_eq!(find_root(&mut parent, 4), 4);
    }

    #[test]
    fn boxes() {
        let mut d = Diagram::new("M");
        assert_eq!(box_of(&d, &[]), (0.0, 0.0, 0.0, 0.0));
        d.nodes.push(Node::new("a", "A", 20.0, 10.0));
        d.nodes.push(Node::new("b", "A", 20.0, 10.0));
        d.nodes[1].x = 40.0;
        d.nodes[1].y = -30.0;
        assert_eq!(box_of(&d, &[0, 1]), (-10.0, -35.0, 50.0, 5.0));
        assert_eq!(box_of(&d, &[1]), (30.0, -35.0, 50.0, -25.0));
    }

    #[test]
    fn chain_left_to_right() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c"] {
            d.nodes.push(Node::new(name, "Gain", 20.0, 20.0));
        }
        d.links.push(link(0, None, 1, None));
        d.links.push(link(1, None, 2, None));
        place_diagram(&mut d, &Options::default());
        assert_eq!(origins(&d), vec![(-50.0, 0.0), (0.0, 0.0), (50.0, 0.0)]);
        assert!(d.nodes.iter().all(|n| n.placed && n.rot == 0));
    }

    #[test]
    fn chain_down() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c"] {
            d.nodes.push(Node::new(name, "Gain", 20.0, 20.0));
        }
        d.links.push(link(0, None, 1, None));
        d.links.push(link(1, None, 2, None));
        let opts = Options {
            direction: "down".to_owned(),
            ..Options::default()
        };
        // Layers 40 apart (ceil((10 + 20 + 10) / 10) * 10), centred.
        place_diagram(&mut d, &opts);
        assert_eq!(origins(&d), vec![(0.0, 40.0), (0.0, 0.0), (0.0, -40.0)]);
    }

    #[test]
    fn unconnected_nodes_in_a_grid() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c"] {
            d.nodes.push(Node::new(name, "A", 20.0, 20.0));
        }
        // Two columns 50 apart, rows 40 apart; the box (0, -60, 70, 0) is moved by (-40, 30).
        place_diagram(&mut d, &Options::default());
        assert_eq!(origins(&d), vec![(-30.0, 20.0), (20.0, 20.0), (-30.0, -20.0)]);
        let grid = Options {
            layout: "grid".to_owned(),
            ..Options::default()
        };
        d.nodes.push(Node::new("d", "A", 20.0, 20.0));
        place_diagram(&mut d, &grid);
        assert_eq!(
            origins(&d),
            vec![(-30.0, 20.0), (20.0, 20.0), (-30.0, -20.0), (20.0, -20.0)]
        );
    }

    #[test]
    fn centre_rounds_ties_to_even() {
        let mut d = Diagram::new("M");
        d.nodes.push(Node::new("a", "A", 50.0, 20.0));
        // The box is (0, -20, 50, 0): 50 / 20 = 2.5 rounds to 2.
        place_diagram(&mut d, &Options::default());
        assert_eq!(origins(&d), vec![(5.0, 0.0)]);
    }

    #[test]
    fn fixed_nodes_stay() {
        let mut d = Diagram::new("M");
        d.nodes.push(Node::new("a", "A", 20.0, 20.0));
        d.nodes[0].fixed = true;
        d.nodes[0].placed = true;
        d.nodes[0].x = 7.0;
        place_diagram(&mut d, &Options::default());
        assert_eq!((d.nodes[0].x, d.nodes[0].y), (7.0, 0.0));
    }

    #[test]
    fn incremental_next_to_the_fixed_node() {
        let mut d = Diagram::new("M");
        d.nodes.push(Node::new("a", "A", 20.0, 20.0));
        d.nodes.push(Node::new("b", "A", 20.0, 20.0));
        d.nodes.push(Node::new("c", "A", 20.0, 20.0));
        d.nodes[0].fixed = true;
        d.nodes[0].placed = true;
        d.links.push(link(0, None, 1, None));
        let opts = Options {
            keep: true,
            ..Options::default()
        };
        place_diagram(&mut d, &opts);
        // b goes below a: 0 - (20 + 20) / 2 - 20. c connects to nothing: right of the fixed nodes,
        // at 10 + 2 * 30 + 10, with its top at theirs.
        assert_eq!(origins(&d)[1..], [(0.0, -40.0), (80.0, 0.0)]);
    }

    #[test]
    fn sign_of_zero() {
        assert_eq!(sign(-3.0), -1.0);
        assert_eq!(sign(2.0), 1.0);
        assert!(sign(0.0) == 0.0 && sign(0.0).is_sign_positive());
        assert!(sign(-0.0) == 0.0 && sign(-0.0).is_sign_negative());
    }

    #[test]
    fn two_pin_part_turns_to_its_neighbours() {
        let mut d = Diagram::new("M");
        let mut r = Node::new("r", "Resistor", 40.0, 20.0);
        r.add_port(port("p", -100.0, 0.0));
        r.add_port(port("n", 100.0, 0.0));
        d.nodes.push(r);
        d.nodes.push(Node::new("up", "A", 20.0, 20.0));
        d.nodes.push(Node::new("down", "A", 20.0, 20.0));
        d.nodes[1].y = 50.0;
        d.nodes[2].y = -50.0;
        d.links.push(link(0, Some(0), 1, None));
        d.links.push(link(0, Some(1), 2, None));
        // Costs: 120 at 0, 161 at 90, 121 at 180, 81 at 270 degrees.
        assert!(choose_rotations(&mut d));
        assert_eq!((d.nodes[0].rot, d.nodes[0].w, d.nodes[0].h), (270, 20.0, 40.0));
        assert!(!choose_rotations(&mut d));
    }
}
