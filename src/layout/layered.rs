// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Layered placement of one connected group of nodes: the signal flows from layer to layer.
//!
//! 1. Every link gets a direction: from output to input; for acausal connectors from the side that
//!    faces the flow direction to the opposite one; otherwise in declaration order.
//! 2. Links are added in the order of how sure their direction is; one that would close a cycle is
//!    reversed. Layers follow from the longest path.
//! 3. The order within the layers is improved by barycenter sweeps; the order with the fewest
//!    crossings is kept.
//! 4. Along the layers, nodes are aligned with the connectors of their neighbours, as far as the
//!    minimum distance between nodes allows.
//!
//! Layers are numbered from 1, as in the Julia source; 0 is a node outside the group.
//! Mirrors `src/layered.jl` of ModelicaComponentAutolayout.jl.

use std::collections::HashMap;

use crate::layout::diagram::{BOTTOM, Diagram, INPUT, LEFT, Node, OUTPUT, RIGHT, TOP};
use crate::numbers::{float_order, sort_perm};
use crate::options::Options;

/// The role of a link end in the flow: positive for an end the flow leaves, negative for one it enters.
fn end_role(n: &Node, pi: Option<usize>, down: bool) -> i32 {
    if n.is_connector {
        return match n.causality {
            INPUT => 2,
            OUTPUT => -2,
            _ => 0,
        };
    }
    let Some(p) = pi.and_then(|i| n.ports.get(i)) else {
        return 0;
    };
    if p.causality == OUTPUT {
        return 2;
    }
    if p.causality == INPUT {
        return -2;
    }
    let s = n.port_side(pi);
    if down {
        return match s {
            BOTTOM => 1,
            TOP => -1,
            _ => 0,
        };
    }
    match s {
        RIGHT => 1,
        LEFT => -1,
        _ => 0,
    }
}

/// True when `to` can be reached from `from` along `succ`.
fn reaches(succ: &[Vec<usize>], from: usize, to: usize, seen: &mut [usize], stamp: usize) -> bool {
    if from == to {
        return true;
    }
    let mut stack: Vec<usize> = vec![from];
    seen[from] = stamp;
    while let Some(u) = stack.pop() {
        for &v in &succ[u] {
            if v == to {
                return true;
            }
            if seen[v] != stamp {
                seen[v] = stamp;
                stack.push(v);
            }
        }
    }
    false
}

/// Assign a layer (1, 2, ...) to every member. Returns the number of layers.
fn assign_layers(layer: &mut [usize], d: &Diagram, members: &[usize], link_ids: &[usize], down: bool) -> usize {
    let n = d.nodes.len();
    let mut pair_index: HashMap<(usize, usize), usize> = HashMap::new();
    let mut pu: Vec<usize> = Vec::new();
    let mut pv: Vec<usize> = Vec::new();
    let mut pp: Vec<i32> = Vec::new();
    for &li in link_ids {
        let l = &d.links[li];
        if l.a == l.b {
            continue;
        }
        let ra = end_role(&d.nodes[l.a], l.pa, down);
        let rb = end_role(&d.nodes[l.b], l.pb, down);
        let key = (l.a.min(l.b), l.a.max(l.b));
        let (u, v) = if ra > rb {
            (l.a, l.b)
        } else if ra < rb {
            (l.b, l.a)
        } else {
            key
        };
        let mut pr = 0;
        if ra != rb {
            pr = if ra.abs() == 2 && rb.abs() == 2 {
                3
            } else if ra.abs() == 2 || rb.abs() == 2 {
                2
            } else {
                1
            };
        }
        match pair_index.get(&key) {
            None => {
                pu.push(u);
                pv.push(v);
                pp.push(pr);
                pair_index.insert(key, pu.len() - 1);
            }
            Some(&k) => {
                if pr > pp[k] {
                    pu[k] = u;
                    pv[k] = v;
                    pp[k] = pr;
                }
            }
        }
    }
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut seen: Vec<usize> = vec![0; n];
    let mut stamp = 0_usize;
    for prio in (0..=3).rev() {
        for k in 0..pu.len() {
            if pp[k] != prio {
                continue;
            }
            let mut a = pu[k];
            let mut b = pv[k];
            stamp += 1;
            if reaches(&succ, b, a, &mut seen, stamp) {
                a = pv[k];
                b = pu[k];
            }
            succ[a].push(b);
            pred[b].push(a);
        }
    }
    let mut indeg: Vec<i64> = vec![0; n];
    let mut queue: Vec<usize> = Vec::new();
    for &m in members {
        indeg[m] = pred[m].len() as i64;
        layer[m] = 1;
        if indeg[m] == 0 {
            queue.push(m);
        }
    }
    let mut head = 0;
    while head < queue.len() {
        let q = queue[head];
        head += 1;
        for &s in &succ[q] {
            layer[s] = layer[s].max(layer[q] + 1);
            indeg[s] -= 1;
            if indeg[s] == 0 {
                queue.push(s);
            }
        }
    }
    let mut nl = 1_usize;
    for &m in members {
        if pred[m].is_empty() && !succ[m].is_empty() {
            let mut lo = usize::MAX;
            for &s in &succ[m] {
                lo = lo.min(layer[s]);
            }
            // The successors of a source are in layer 2 or later.
            layer[m] = lo - 1;
        }
        nl = nl.max(layer[m]);
    }
    nl
}

/// The offset of a port across the layer, positive towards later positions.
fn cross_offset(n: &Node, pi: Option<usize>, down: bool) -> f64 {
    let o = n.port_offset(pi);
    if down { o.0 } else { -o.1 }
}

fn cross_size(n: &Node, down: bool) -> f64 {
    if down { n.w } else { n.h }
}

fn main_size(n: &Node, down: bool) -> f64 {
    if down { n.h } else { n.w }
}

/// The offset of a port across the layer as a fraction of half the node's size across it.
fn port_fraction(n: &Node, pi: Option<usize>, down: bool) -> f64 {
    cross_offset(n, pi, down) / (cross_size(n, down) / 2.0).max(1.0e-9)
}

/// The number of link pairs between the same two layers that cross, for the positions `pos`.
fn count_crossings(d: &Diagram, link_ids: &[usize], layer: &[usize], pos: &[f64], down: bool) -> usize {
    let mut la: Vec<usize> = Vec::new();
    let mut lb: Vec<usize> = Vec::new();
    let mut qa: Vec<f64> = Vec::new();
    let mut qb: Vec<f64> = Vec::new();
    for &li in link_ids {
        let l = &d.links[li];
        if layer[l.a] == layer[l.b] {
            continue;
        }
        let fa = pos[l.a] + 0.3 * port_fraction(&d.nodes[l.a], l.pa, down);
        let fb = pos[l.b] + 0.3 * port_fraction(&d.nodes[l.b], l.pb, down);
        if layer[l.a] < layer[l.b] {
            la.push(layer[l.a]);
            lb.push(layer[l.b]);
            qa.push(fa);
            qb.push(fb);
        } else {
            la.push(layer[l.b]);
            lb.push(layer[l.a]);
            qa.push(fb);
            qb.push(fa);
        }
    }
    let mut count = 0;
    for i in 0..la.len() {
        for j in (i + 1)..la.len() {
            if la[i] == la[j] && lb[i] == lb[j] && (qa[i] - qa[j]) * (qb[i] - qb[j]) < 0.0 {
                count += 1;
            }
        }
    }
    count
}

/// Order the nodes within the layers. Returns the layers, each in its order.
fn order_layers(
    d: &Diagram,
    members: &[usize],
    link_ids: &[usize],
    inc: &[Vec<usize>],
    layer: &[usize],
    nl: usize,
    down: bool,
) -> Vec<Vec<usize>> {
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); nl];
    // Positions count from 1, as in Julia: they enter the barycenters as numbers.
    let mut pos: Vec<f64> = vec![0.0; d.nodes.len()];
    for &m in members {
        layers[layer[m] - 1].push(m);
        pos[m] = layers[layer[m] - 1].len() as f64;
    }
    let counting = link_ids.len() <= 2000;
    let mut best: Vec<Vec<usize>> = layers.clone();
    let mut best_count = if counting {
        count_crossings(d, link_ids, layer, &pos, down)
    } else {
        0
    };
    for sweep in 1..=8 {
        if counting && best_count == 0 {
            break;
        }
        let forward = sweep % 2 == 1;
        let ks: Vec<usize> = if forward {
            (2..=nl).collect()
        } else {
            (1..nl).rev().collect()
        };
        for k in ks {
            let nodes = &layers[k - 1];
            let mut bary: Vec<f64> = Vec::with_capacity(nodes.len());
            for &m in nodes {
                let mut total = 0.0_f64;
                let mut cnt = 0_i64;
                for &li in &inc[m] {
                    let l = &d.links[li];
                    let (o, po, _) = l.seen_from(m);
                    if o == m || layer[o] == k || (layer[o] > k) == forward {
                        continue;
                    }
                    total += pos[o] + 0.3 * port_fraction(&d.nodes[o], po, down);
                    cnt += 1;
                }
                bary.push(if cnt == 0 { pos[m] } else { total / cnt as f64 });
            }
            let sorted: Vec<usize> = sort_perm(&bary).into_iter().map(|p| nodes[p]).collect();
            for (i, &m) in sorted.iter().enumerate() {
                pos[m] = (i + 1) as f64;
            }
            layers[k - 1] = sorted;
        }
        if counting {
            let c = count_crossings(d, link_ids, layer, &pos, down);
            if c < best_count {
                best_count = c;
                best = layers.clone();
            }
        } else {
            best = layers.clone();
        }
    }
    best
}

/// Least-squares fit of a non-decreasing sequence to `z` (pool adjacent violators).
fn isotonic(z: &[f64]) -> Vec<f64> {
    let mut sums: Vec<f64> = Vec::new();
    let mut counts: Vec<i64> = Vec::new();
    for &x in z {
        sums.push(x);
        counts.push(1);
        while let [.., s0, s1] = sums[..]
            && let [.., c0, c1] = counts[..]
            && s0 / c0 as f64 > s1 / c1 as f64
        {
            sums.pop();
            counts.pop();
            if let (Some(s), Some(c)) = (sums.last_mut(), counts.last_mut()) {
                *s += s1;
                *c += c1;
            }
        }
    }
    let mut out: Vec<f64> = Vec::with_capacity(z.len());
    for (s, &c) in sums.iter().zip(&counts) {
        for _ in 0..c {
            out.push(s / c as f64);
        }
    }
    out
}

/// Set the positions `c` across layer `k`: each node as near as possible to the connectors of its
/// neighbours. `mode` -1 uses the neighbours in earlier layers, 1 those in later layers, 0 all.
/// Returns false when no node of the layer has such a neighbour.
#[allow(clippy::too_many_arguments)]
fn align_layer(
    c: &mut [f64],
    d: &Diagram,
    nodes: &[usize],
    inc: &[Vec<usize>],
    layer: &[usize],
    k: usize,
    mode: i32,
    down: bool,
    gap: f64,
) -> bool {
    let n = nodes.len();
    let mut des: Vec<f64> = vec![f64::NAN; n];
    let mut found = false;
    for i in 0..n {
        let m = nodes[i];
        let mut vals: Vec<f64> = Vec::new();
        for &li in &inc[m] {
            let l = &d.links[li];
            let (o, po, pm) = l.seen_from(m);
            if o == m || layer[o] == 0 || layer[o] == k {
                continue;
            }
            if mode == -1 && layer[o] > k {
                continue;
            }
            if mode == 1 && layer[o] < k {
                continue;
            }
            vals.push(c[o] + cross_offset(&d.nodes[o], po, down) - cross_offset(&d.nodes[m], pm, down));
        }
        if !vals.is_empty() {
            vals.sort_by(float_order);
            des[i] = vals[vals.len().div_ceil(2) - 1];
            found = true;
        }
    }
    if !found {
        return false;
    }
    let mut s: Vec<f64> = vec![0.0; n];
    for i in 1..n {
        s[i] = s[i - 1] + (cross_size(&d.nodes[nodes[i - 1]], down) + cross_size(&d.nodes[nodes[i]], down)) / 2.0 + gap;
    }
    let mut want: Vec<f64> = des.clone();
    for i in 1..n {
        if want[i].is_nan() && !want[i - 1].is_nan() {
            want[i] = want[i - 1] + s[i] - s[i - 1];
        }
    }
    for i in (0..n - 1).rev() {
        if want[i].is_nan() && !want[i + 1].is_nan() {
            want[i] = want[i + 1] - (s[i + 1] - s[i]);
        }
    }
    let shifted: Vec<f64> = (0..n).map(|i| want[i] - s[i]).collect();
    let fit = isotonic(&shifted);
    for i in 0..n {
        let mut x = fit[i] + s[i];
        if !des[i].is_nan() && (x - des[i]).abs() < 0.01 {
            x = (des[i] * 100.0).round_ties_even() / 100.0;
        } else {
            x = (x / 2.0).round_ties_even() * 2.0;
        }
        if i > 0 {
            x = x.max(c[nodes[i - 1]] + s[i] - s[i - 1]);
        }
        c[nodes[i]] = x;
    }
    true
}

/// Stack the nodes of a layer, centred at 0.
fn stack_layer(c: &mut [f64], d: &Diagram, nodes: &[usize], down: bool, gap: f64) {
    let mut total = 0.0_f64;
    for &m in nodes {
        total += cross_size(&d.nodes[m], down);
    }
    total += gap * nodes.len().saturating_sub(1) as f64;
    let mut at = -total / 2.0;
    for &m in nodes {
        let sz = cross_size(&d.nodes[m], down);
        c[m] = ((at + sz / 2.0) / 2.0).round_ties_even() * 2.0;
        at += sz + gap;
    }
}

/// Place the nodes `members`, connected by the links `link_ids`, in layers.
pub fn layer_component(d: &mut Diagram, members: &[usize], link_ids: &[usize], inc: &[Vec<usize>], opts: &Options) {
    let down = opts.direction == "down";
    let gap_main = if down { opts.dy } else { opts.dx };
    let gap_cross = if down { opts.dx } else { opts.dy };
    let mut layer: Vec<usize> = vec![0; d.nodes.len()];
    let nl = assign_layers(&mut layer, d, members, link_ids, down);
    let layers = order_layers(d, members, link_ids, inc, &layer, nl, down);
    let mut c: Vec<f64> = vec![0.0; d.nodes.len()];
    stack_layer(&mut c, d, &layers[0], down, gap_cross);
    for k in 2..=nl {
        if !align_layer(&mut c, d, &layers[k - 1], inc, &layer, k, -1, down, gap_cross) {
            stack_layer(&mut c, d, &layers[k - 1], down, gap_cross);
        }
    }
    for _ in 0..2 {
        for k in (1..nl).rev() {
            align_layer(&mut c, d, &layers[k - 1], inc, &layer, k, 0, down, gap_cross);
        }
        for k in 2..=nl {
            align_layer(&mut c, d, &layers[k - 1], inc, &layer, k, 0, down, gap_cross);
        }
    }
    let mut thick: Vec<f64> = vec![0.0; nl];
    for &m in members {
        thick[layer[m] - 1] = thick[layer[m] - 1].max(main_size(&d.nodes[m], down));
    }
    // span[k - 1]: the links that pass between the layers k and k + 1.
    let mut span: Vec<i64> = vec![0; nl];
    for &li in link_ids {
        let l = &d.links[li];
        for k in layer[l.a].min(layer[l.b]).max(1)..layer[l.a].max(layer[l.b]) {
            span[k - 1] += 1;
        }
    }
    let mut main: Vec<f64> = vec![0.0; nl];
    for k in 1..nl {
        let g = gap_main.max(10.0 + 5.0 * span[k - 1] as f64).min(gap_main.max(80.0));
        main[k] = ((main[k - 1] + thick[k - 1] / 2.0 + g + thick[k] / 2.0) / 10.0).ceil() * 10.0;
    }
    for &m in members {
        let node = &mut d.nodes[m];
        if down {
            node.x = c[m];
            node.y = -main[layer[m] - 1];
        } else {
            node.x = main[layer[m] - 1];
            node.y = -c[m];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::diagram::{ACAUSAL, Link, Port};

    fn port(name: &str, ix: f64, iy: f64, causality: i32) -> Port {
        Port {
            name: name.to_owned(),
            ix,
            iy,
            known: true,
            causality,
            color: String::new(),
        }
    }

    /// A 20 x 20 block: input `u` on the left, output `y` on the right, pins `p` (top) and `n` (bottom).
    fn block(name: &str) -> Node {
        let mut n = Node::new(name, "Block", 20.0, 20.0);
        n.add_port(port("u", -100.0, 0.0, INPUT));
        n.add_port(port("y", 100.0, 0.0, OUTPUT));
        n.add_port(port("p", 0.0, 100.0, ACAUSAL));
        n.add_port(port("n", 0.0, -100.0, ACAUSAL));
        n
    }

    fn incidence(d: &Diagram) -> Vec<Vec<usize>> {
        let mut inc = vec![Vec::new(); d.nodes.len()];
        for (li, l) in d.links.iter().enumerate() {
            inc[l.a].push(li);
            inc[l.b].push(li);
        }
        inc
    }

    #[test]
    fn roles() {
        let n = block("a");
        assert_eq!(end_role(&n, None, false), 0);
        assert_eq!(end_role(&n, Some(9), false), 0);
        assert_eq!(end_role(&n, Some(0), false), -2);
        assert_eq!(end_role(&n, Some(1), true), 2);
        assert_eq!(end_role(&n, Some(2), false), 0);
        assert_eq!(end_role(&n, Some(2), true), -1);
        assert_eq!(end_role(&n, Some(3), true), 1);
        let mut c = Node::new("u", "RealInput", 20.0, 20.0);
        c.is_connector = true;
        c.causality = INPUT;
        assert_eq!(end_role(&c, None, false), 2);
        c.causality = OUTPUT;
        assert_eq!(end_role(&c, None, false), -2);
    }

    #[test]
    fn reachable() {
        let succ = vec![vec![1], vec![2], vec![], vec![0]];
        let mut seen = vec![0; 4];
        assert!(reaches(&succ, 0, 2, &mut seen, 1));
        assert!(reaches(&succ, 3, 2, &mut seen, 2));
        assert!(!reaches(&succ, 2, 0, &mut seen, 3));
        assert!(!reaches(&succ, 0, 3, &mut seen, 4));
        assert!(reaches(&succ, 2, 2, &mut seen, 5));
    }

    #[test]
    fn layers_follow_the_outputs() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c"] {
            d.nodes.push(block(name));
        }
        // c.y -> b.u, b.y -> a.u: against the declaration order. a.y -> c.u closes the cycle.
        d.links.push(Link::new(1, Some(0), 2, Some(1)));
        d.links.push(Link::new(0, Some(0), 1, Some(1)));
        d.links.push(Link::new(0, Some(1), 2, Some(0)));
        let mut layer = vec![0; 3];
        assert_eq!(assign_layers(&mut layer, &d, &[0, 1, 2], &[0, 1, 2], false), 3);
        assert_eq!(layer, vec![3, 2, 1]);
    }

    #[test]
    fn a_source_moves_next_to_its_successor() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c", "s"] {
            d.nodes.push(block(name));
        }
        d.links.push(Link::new(0, Some(1), 1, Some(0)));
        d.links.push(Link::new(1, Some(1), 2, Some(0)));
        d.links.push(Link::new(3, Some(1), 2, Some(0)));
        let mut layer = vec![0; 4];
        assert_eq!(assign_layers(&mut layer, &d, &[0, 1, 2, 3], &[0, 1, 2], false), 3);
        assert_eq!(layer, vec![1, 2, 3, 2]);
    }

    #[test]
    fn crossings_are_removed() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c", "e"] {
            d.nodes.push(block(name));
        }
        // a -> e and b -> c: the second layer is declared in the crossing order.
        d.links.push(Link::new(0, Some(1), 3, Some(0)));
        d.links.push(Link::new(1, Some(1), 2, Some(0)));
        let layer = vec![1, 1, 2, 2];
        let pos = vec![1.0, 2.0, 1.0, 2.0];
        assert_eq!(count_crossings(&d, &[0, 1], &layer, &pos, false), 1);
        let inc = incidence(&d);
        let layers = order_layers(&d, &[0, 1, 2, 3], &[0, 1], &inc, &layer, 2, false);
        assert_eq!(layers, vec![vec![0, 1], vec![3, 2]]);
    }

    #[test]
    fn pool_adjacent_violators() {
        assert_eq!(isotonic(&[]), Vec::<f64>::new());
        assert_eq!(isotonic(&[1.0, 2.0, 4.0]), vec![1.0, 2.0, 4.0]);
        assert_eq!(isotonic(&[1.0, 3.0, 2.0, 4.0]), vec![1.0, 2.5, 2.5, 4.0]);
        assert_eq!(isotonic(&[3.0, 2.0, 1.0]), vec![2.0, 2.0, 2.0]);
        assert_eq!(isotonic(&[4.0, 0.0, 5.0, 1.0, 3.0]), vec![2.0, 2.0, 3.0, 3.0, 3.0]);
    }

    #[test]
    fn stacked() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c"] {
            d.nodes.push(block(name));
        }
        let mut c = vec![0.0; 3];
        // Total 3 * 20 + 2 * 20 = 100: centres at -40, 0, 40.
        stack_layer(&mut c, &d, &[2, 0, 1], false, 20.0);
        assert_eq!(c, vec![0.0, 40.0, -40.0]);
    }

    #[test]
    fn aligned_with_the_connectors() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c"] {
            d.nodes.push(block(name));
        }
        // a.y -> b.u and a.y -> c.u: b and c both want to be at 0 across the layer.
        d.links.push(Link::new(0, Some(1), 1, Some(0)));
        d.links.push(Link::new(0, Some(1), 2, Some(0)));
        let inc = incidence(&d);
        let layer = vec![1, 2, 2];
        let mut c = vec![0.0; 3];
        assert!(align_layer(&mut c, &d, &[1, 2], &inc, &layer, 2, -1, false, 20.0));
        // The fit is -20, 20 (the mean of 0 and 0 - 40, plus the distances 0 and 40).
        assert_eq!(c, vec![0.0, -20.0, 20.0]);
        assert!(!align_layer(&mut c, &d, &[1, 2], &inc, &layer, 2, 1, false, 20.0));
    }

    #[test]
    fn fan_out_to_the_right() {
        let mut d = Diagram::new("M");
        for name in ["a", "b", "c"] {
            d.nodes.push(block(name));
        }
        d.links.push(Link::new(0, Some(1), 1, Some(0)));
        d.links.push(Link::new(0, Some(1), 2, Some(0)));
        let inc = incidence(&d);
        layer_component(&mut d, &[0, 1, 2], &[0, 1], &inc, &Options::default());
        // Layers 50 apart (10 + 30 + 10); b above c, 40 apart; a between them. Each of the two
        // rounds of alignment moves the group by 20: a goes to its upper neighbour, b above it.
        let at: Vec<(f64, f64)> = d.nodes.iter().map(|n| (n.x, n.y)).collect();
        assert_eq!(at, vec![(0.0, 40.0), (50.0, 60.0), (50.0, 20.0)]);
    }
}
