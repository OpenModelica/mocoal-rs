// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The diagram of one class: nodes (components and the class's own connectors), their ports, and
//! links (connect equations). Placement and routing work on this, independent of the syntax tree.
//! Mirrors `src/diagram.jl` of ModelicaComponentAutolayout.jl.

use std::collections::HashMap;

use crate::absyn::{Transformation, rotate_point};
use crate::tokens::Segment;

pub type Point = (f64, f64);

// The causality of a connector.
pub const ACAUSAL: i32 = 0;
pub const INPUT: i32 = 1;
pub const OUTPUT: i32 = 2;

// The sides of a node, and the directions of the router; 0 is none.
pub const LEFT: i32 = 1;
pub const RIGHT: i32 = 2;
pub const BOTTOM: i32 = 3;
pub const TOP: i32 = 4;

/// Whether `side` is `LEFT` or `RIGHT`, where lines leave horizontally. True for none (0) too.
pub fn is_horizontal(side: i32) -> bool {
    side <= RIGHT
}

/// A connector of a node's class. `ix`, `iy` is its centre in the icon coordinates of that class.
#[derive(Clone, Debug)]
pub struct Port {
    pub name: String,
    pub ix: f64,
    pub iy: f64,
    /// False when the class or the connector's placement could not be read.
    pub known: bool,
    pub causality: i32,
    /// Line colour `{r,g,b}` of the connector class, or "".
    pub color: String,
}

/// A component of the class, or one of its connectors.
///
/// `x, y, w, h` is the bounding box (centre and size) in diagram coordinates. `ox` to `rotation`
/// is the `transformation` that gives it. `cx1` to `cy2` is the icon coordinate system of the
/// node's class, onto which the extent is mapped.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub type_name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub ox: f64,
    pub oy: f64,
    pub ex1: f64,
    pub ey1: f64,
    pub ex2: f64,
    pub ey2: f64,
    pub rotation: f64,
    pub cx1: f64,
    pub cy1: f64,
    pub cx2: f64,
    pub cy2: f64,
    pub preserve_aspect_ratio: bool,
    pub placed: bool,
    /// A fixed node keeps its transformation.
    pub fixed: bool,
    /// A connector of the class itself.
    pub is_connector: bool,
    pub causality: i32,
    pub color: String,
    pub ports: Vec<Port>,
    pub port_index: HashMap<String, usize>,
    /// The declaration in the source; none for an inherited node.
    pub seg: Option<Segment>,
    /// The `annotation` token that receives the Placement, when the declaration has one.
    pub target: Option<usize>,
    pub has_placement: bool,
    pub has_transformation: bool,
    pub has_icon_transformation: bool,
    /// Declared in a base class: it cannot be annotated here.
    pub inherited: bool,
    /// The type did not resolve: kept only when a connect equation or a Placement uses it.
    pub tentative: bool,
    /// The rotation in degrees (0, 90, 180 or 270) that the layout chose.
    pub rot: i32,
    /// The node had a placement: `old_x`, `old_y` is the centre of its box there.
    pub has_old: bool,
    pub old_x: f64,
    pub old_y: f64,
}

impl Node {
    /// A node `name` of the class `type_name`, `w` wide and `h` high, not placed yet.
    pub fn new(name: &str, type_name: &str, w: f64, h: f64) -> Node {
        Node {
            name: name.to_owned(),
            type_name: type_name.to_owned(),
            x: 0.0,
            y: 0.0,
            w,
            h,
            ox: 0.0,
            oy: 0.0,
            ex1: -w / 2.0,
            ey1: -h / 2.0,
            ex2: w / 2.0,
            ey2: h / 2.0,
            rotation: 0.0,
            cx1: -100.0,
            cy1: -100.0,
            cx2: 100.0,
            cy2: 100.0,
            preserve_aspect_ratio: true,
            placed: false,
            fixed: false,
            is_connector: false,
            causality: ACAUSAL,
            color: String::new(),
            ports: Vec::new(),
            port_index: HashMap::new(),
            seg: None,
            target: None,
            has_placement: false,
            has_transformation: false,
            has_icon_transformation: false,
            inherited: false,
            tentative: false,
            rot: 0,
            has_old: false,
            old_x: 0.0,
            old_y: 0.0,
        }
    }

    /// Add the port to the node, or return the index of the one it has by that name.
    pub fn add_port(&mut self, p: Port) -> usize {
        if let Some(&i) = self.port_index.get(&p.name) {
            return i;
        }
        self.port_index.insert(p.name.clone(), self.ports.len());
        self.ports.push(p);
        self.ports.len() - 1
    }

    fn turned(&self) -> bool {
        (self.rot / 90) % 2 != 0
    }

    /// The width of the node's extent, before the rotation (`w` is the width of the box that results).
    pub fn extent_width(&self) -> f64 {
        if self.turned() { self.h } else { self.w }
    }

    /// The height of the node's extent, before the rotation.
    pub fn extent_height(&self) -> f64 {
        if self.turned() { self.w } else { self.h }
    }

    /// Give a free node the transformation of its bounding box: origin at the centre, its rotation.
    pub fn set_free(&mut self) {
        self.ox = self.x;
        self.oy = self.y;
        let ew = self.extent_width();
        let eh = self.extent_height();
        self.ex1 = -ew / 2.0;
        self.ey1 = -eh / 2.0;
        self.ex2 = ew / 2.0;
        self.ey2 = eh / 2.0;
        self.rotation = f64::from(self.rot);
    }

    /// Rotate the node to `r` degrees (a multiple of 90): the box turns with it.
    pub fn set_rotation(&mut self, r: i32) {
        if ((r / 90) % 2 != 0) != self.turned() {
            std::mem::swap(&mut self.w, &mut self.h);
        }
        self.rot = r;
    }

    /// Give a node the transformation `t` and the bounding box that follows from it.
    pub fn set_transformation(&mut self, t: &Transformation) {
        self.ox = t.ox;
        self.oy = t.oy;
        self.ex1 = t.ex1;
        self.ey1 = t.ey1;
        self.ex2 = t.ex2;
        self.ey2 = t.ey2;
        self.rotation = t.rotation;
        let (mut xmin, mut xmax, mut ymin, mut ymax) =
            (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for (cx, cy) in [(t.ex1, t.ey1), (t.ex1, t.ey2), (t.ex2, t.ey1), (t.ex2, t.ey2)] {
            let p = rotate_point(cx, cy, t.rotation);
            xmin = xmin.min(p.0);
            xmax = xmax.max(p.0);
            ymin = ymin.min(p.1);
            ymax = ymax.max(p.1);
        }
        self.x = t.ox + (xmin + xmax) / 2.0;
        self.y = t.oy + (ymin + ymax) / 2.0;
        self.w = (xmax - xmin).max(1.0);
        self.h = (ymax - ymin).max(1.0);
    }

    fn known_port(&self, port: Option<usize>) -> Option<&Port> {
        port.and_then(|i| self.ports.get(i)).filter(|p| p.known)
    }

    /// The position of a port of the node in diagram coordinates (the centre for no port).
    pub fn port_position(&self, port: Option<usize>) -> Point {
        let Some(p) = self.known_port(port) else {
            return (self.x, self.y);
        };
        let cw = self.cx2 - self.cx1;
        let ch = self.cy2 - self.cy1;
        if cw == 0.0 || ch == 0.0 {
            return (self.x, self.y);
        }
        let mut sx = (self.ex2 - self.ex1) / cw;
        let mut sy = (self.ey2 - self.ey1) / ch;
        if self.preserve_aspect_ratio {
            let m = sx.abs().min(sy.abs());
            sx = if sx < 0.0 { -m } else { m };
            sy = if sy < 0.0 { -m } else { m };
        }
        let lx = (self.ex1 + self.ex2) / 2.0 + sx * (p.ix - (self.cx1 + self.cx2) / 2.0);
        let ly = (self.ey1 + self.ey2) / 2.0 + sy * (p.iy - (self.cy1 + self.cy2) / 2.0);
        let r = rotate_point(lx, ly, self.rotation);
        (self.ox + r.0, self.oy + r.1)
    }

    /// The position of a port relative to the centre of the node, when the node is turned to `r`
    /// degrees. For a node that is not fixed: extent around the origin.
    pub fn port_offset_at(&self, port: Option<usize>, r: i32) -> Point {
        let Some(p) = self.known_port(port) else {
            return (0.0, 0.0);
        };
        let cw = self.cx2 - self.cx1;
        let ch = self.cy2 - self.cy1;
        if cw == 0.0 || ch == 0.0 {
            return (0.0, 0.0);
        }
        let mut sx = self.extent_width() / cw.abs();
        let mut sy = self.extent_height() / ch.abs();
        if self.preserve_aspect_ratio {
            sx = sx.min(sy);
            sy = sx;
        }
        rotate_point(
            sx * (p.ix - (self.cx1 + self.cx2) / 2.0),
            sy * (p.iy - (self.cy1 + self.cy2) / 2.0),
            f64::from(r),
        )
    }

    /// The position of a port relative to the centre of the node. For a node that is not fixed this
    /// is the offset it has once the node is placed (extent around the origin, its rotation).
    pub fn port_offset(&self, port: Option<usize>) -> Point {
        if self.known_port(port).is_none() {
            return (0.0, 0.0);
        }
        if self.fixed {
            let q = self.port_position(port);
            return (q.0 - self.x, q.1 - self.y);
        }
        self.port_offset_at(port, self.rot)
    }

    /// The side of the node on which a port lies, 0 when it is near the centre or not known.
    pub fn port_side(&self, port: Option<usize>) -> i32 {
        if self.known_port(port).is_none() {
            return 0;
        }
        let o = self.port_offset(port);
        self.side_of_offset(o.0, o.1)
    }

    /// The side of the node nearest to the point `q`, 0 near the centre.
    pub fn side_of(&self, q: Point) -> i32 {
        self.side_of_offset(q.0 - self.x, q.1 - self.y)
    }

    fn side_of_offset(&self, dx: f64, dy: f64) -> i32 {
        let rx = dx / (self.w / 2.0).max(1.0e-9);
        let ry = dy / (self.h / 2.0).max(1.0e-9);
        if rx.abs().max(ry.abs()) < 0.25 {
            0
        } else if rx.abs() >= ry.abs() {
            if rx < 0.0 { LEFT } else { RIGHT }
        } else if ry < 0.0 {
            BOTTOM
        } else {
            TOP
        }
    }
}

/// A connect equation. `pa`, `pb` index the ports of the nodes `a`, `b`; none is the node itself.
#[derive(Clone, Debug)]
pub struct Link {
    pub a: usize,
    pub pa: Option<usize>,
    pub b: usize,
    pub pb: Option<usize>,
    pub points: Vec<Point>,
    /// Tokens of `connect`, of the `;`, and of the equation's `annotation`.
    pub eq_tok: Option<usize>,
    pub semi_tok: Option<usize>,
    pub ann_tok: Option<usize>,
    pub color: String,
    pub smooth: bool,
    /// The existing Line is kept.
    pub keep: bool,
}

impl Link {
    /// A link between port `pa` of node `a` and port `pb` of node `b`, with no line and no tokens.
    pub fn new(a: usize, pa: Option<usize>, b: usize, pb: Option<usize>) -> Link {
        Link {
            a,
            pa,
            b,
            pb,
            points: Vec::new(),
            eq_tok: None,
            semi_tok: None,
            ann_tok: None,
            color: String::new(),
            smooth: false,
            keep: false,
        }
    }

    /// End `which` (0 is `a`, 1 is `b`): its node and port.
    pub fn end(&self, which: usize) -> (usize, Option<usize>) {
        if which == 0 {
            (self.a, self.pa)
        } else {
            (self.b, self.pb)
        }
    }

    /// Seen from node `m`: the node at the other end, its port, and the port at `m`.
    pub fn seen_from(&self, m: usize) -> (usize, Option<usize>, Option<usize>) {
        if self.a == m {
            (self.b, self.pb, self.pa)
        } else {
            (self.a, self.pa, self.pb)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diagram {
    pub class_name: String,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub index: HashMap<String, usize>,
    /// Token of the `end` of the class's `end Name`.
    pub end_tok: Option<usize>,
    /// Token of the `(` of the class annotation, when the class has one.
    pub ann_lpar: Option<usize>,
    pub has_extent: bool,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    /// The places (x, y, w, h) in the class's icon for its connectors without a Placement, by name.
    pub icon_places: HashMap<String, (f64, f64, f64, f64)>,
}

impl Diagram {
    /// An empty diagram of the class `name`.
    pub fn new(name: &str) -> Diagram {
        Diagram {
            class_name: name.to_owned(),
            nodes: Vec::new(),
            links: Vec::new(),
            index: HashMap::new(),
            end_tok: None,
            ann_lpar: None,
            has_extent: false,
            x1: -100.0,
            y1: -100.0,
            x2: 100.0,
            y2: 100.0,
            icon_places: HashMap::new(),
        }
    }

    /// Place the node at the free position nearest to `c`: positions on a grid of 10 are tried in
    /// rings around `c`, and the first one where the node overlaps none of the nodes `among` is taken.
    pub fn place_near_among(&mut self, i: usize, c: Point, gap: f64, among: &[usize]) {
        'rings: for r in 0..=400_i64 {
            for k in 0..(8 * r).max(1) {
                let (dx, dy) = if r == 0 { (0, 0) } else { ring_step(r, k) };
                self.nodes[i].x = (c.0 / 10.0).round_ties_even() * 10.0 + 10.0 * dx as f64;
                self.nodes[i].y = (c.1 / 10.0).round_ties_even() * 10.0 + 10.0 * dy as f64;
                if among
                    .iter()
                    .all(|&j| j == i || !overlaps(&self.nodes[i], &self.nodes[j], gap))
                {
                    break 'rings;
                }
            }
        }
        self.nodes[i].placed = true;
        self.nodes[i].set_free();
    }

    /// Place the node at the free position nearest to `c`, clear of all placed nodes.
    pub fn place_near(&mut self, i: usize, c: Point, gap: f64) {
        let among: Vec<usize> = (0..self.nodes.len())
            .filter(|&j| j != i && self.nodes[j].placed)
            .collect();
        self.place_near_among(i, c, gap, &among);
    }

    /// The bounding box of the nodes and the link points: (x1, y1, x2, y2).
    pub fn bounding_box(&self) -> (f64, f64, f64, f64) {
        let (mut x1, mut y1, mut x2, mut y2) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for n in &self.nodes {
            x1 = x1.min(n.x - n.w / 2.0);
            x2 = x2.max(n.x + n.w / 2.0);
            y1 = y1.min(n.y - n.h / 2.0);
            y2 = y2.max(n.y + n.h / 2.0);
        }
        for p in self.links.iter().flat_map(|l| &l.points) {
            x1 = x1.min(p.0);
            x2 = x2.max(p.0);
            y1 = y1.min(p.1);
            y2 = y2.max(p.1);
        }
        if x1.is_finite() {
            (x1, y1, x2, y2)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }
}

/// The unit vector that points out of side `s`.
pub fn side_vector(s: i32) -> Point {
    match s {
        LEFT => (-1.0, 0.0),
        RIGHT => (1.0, 0.0),
        BOTTOM => (0.0, -1.0),
        TOP => (0.0, 1.0),
        _ => (0.0, 0.0),
    }
}

/// True when the boxes of two nodes, widened by `gap`, overlap.
pub fn overlaps(a: &Node, b: &Node, gap: f64) -> bool {
    (a.x - b.x).abs() < (a.w + b.w) / 2.0 + gap && (a.y - b.y).abs() < (a.h + b.h) / 2.0 + gap
}

/// The k-th cell of the square ring at distance r (0 <= k < 8r), starting above.
fn ring_step(r: i64, k: i64) -> (i64, i64) {
    let o = k % (2 * r);
    match k / (2 * r) {
        0 => (-r + o, r),
        1 => (r, r - o),
        2 => (r - o, -r),
        _ => (-r, -r + o),
    }
}
