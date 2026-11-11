// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The shapes of a class's Diagram layer, `Diagram(graphics={...})`: frames, texts, arrows and
//! junction markers drawn around the components. They were placed for the old layout, so a new
//! layout has to move them or drop them.
//!
//! `move` (the default):
//! - a rectangle that holds components becomes the frame of those components in the new layout;
//!   when that frame would cover another component, it is dropped;
//! - a shape inside such a frame, or a text up to 25 above it (a title), moves with the frame,
//!   and goes with it when it is dropped;
//! - a small rectangle or ellipse (a junction marker) is dropped: the lines it marked are new; so
//!   is a large rectangle that holds no component;
//! - any other shape (a note, an arrow) moves as the component nearest to it did.
//!
//! `drop` removes all shapes, `keep` leaves them.
//! Mirrors `src/graphics.jl` of ModelicaComponentAutolayout.jl.

use openmodelica_ast::parser::lexer::TokenKind;

use crate::layout::diagram::Diagram;
use crate::numbers::parse_float;
use crate::options::Options;
use crate::source::tokens::{Arg, Edit, Source};
use crate::write::annotate::{arg_lpar, diagram_extent, extent_text, find_arg, point_text, set_extent};

/// The space between a frame and the components it holds.
const FRAME_MARGIN: f64 = 12.0;
/// A rectangle at least this wide and high can be a frame.
const MIN_FRAME_SIZE: f64 = 16.0;
/// A rectangle or ellipse at most this wide and high is a junction marker.
const MAX_MARKER_SIZE: f64 = 12.0;
/// Points closer than this are the same.
const SAME_POINT: f64 = 0.005;
/// How far above a frame a text still counts as its title.
const FRAME_TITLE: f64 = 25.0;

type FrameBox = (f64, f64, f64, f64);

/// A shape of the graphics list: its tokens, the place of its coordinates, and the coordinates.
struct Shape {
    kind: String,
    first: usize,
    last: usize,
    /// The tokens of the `extent` or `points` argument; none when the shape has none that can
    /// be read.
    coord: Option<(usize, usize)>,
    is_extent: bool,
    ox: f64,
    oy: f64,
    /// Coordinates in the diagram (the origin of the shape is added).
    xs: Vec<f64>,
    ys: Vec<f64>,
}

/// The numbers in `s`, in order. A sign in front of a digit belongs to the number.
pub fn scan_numbers(s: &str) -> Vec<f64> {
    let cu = s.as_bytes();
    let n = cu.len();
    let digit = |k: usize| k < n && cu[k].is_ascii_digit();
    let sign = |k: usize| k < n && matches!(cu[k], b'-' | b'+');
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let c = cu[i];
        let lead = matches!(c, b'-' | b'+' | b'.') && i + 1 < n && (digit(i + 1) || cu[i + 1] == b'.');
        if !(c.is_ascii_digit() || lead) {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < n {
            let e = cu[j];
            if e.is_ascii_digit() || e == b'.' {
                j += 1;
            } else if matches!(e, b'e' | b'E') && j + 1 < n && (digit(j + 1) || (sign(j + 1) && digit(j + 2))) {
                j += 2;
            } else {
                break;
            }
        }
        if let Some(v) = parse_float(&s[i..j]) {
            out.push(v);
        }
        i = j;
    }
    out
}

/// The shape of one item of the graphics list, with its coordinates when they can be read.
fn read_shape(src: &Source, item: &Arg) -> Shape {
    let mut sh = Shape {
        kind: item.name.clone(),
        first: item.first,
        last: item.last,
        coord: None,
        is_extent: false,
        ox: 0.0,
        oy: 0.0,
        xs: Vec::new(),
        ys: Vec::new(),
    };
    let Some(lpar) = arg_lpar(src, Some(item)) else {
        return sh;
    };
    let Some((args, rpar)) = src.args_of(lpar) else {
        return sh;
    };
    if rpar != item.last {
        return sh;
    }
    for a in &args {
        if !matches!(a.name.as_str(), "extent" | "points" | "origin") {
            continue;
        }
        let text = src.token_slice(a.first, a.last);
        let Some(eq) = text.find('=') else {
            continue;
        };
        let nums = scan_numbers(&text[eq + 1..]);
        if a.name == "origin" {
            if nums.len() == 2 {
                sh.ox = nums[0];
                sh.oy = nums[1];
            }
        } else if a.name == "extent" && nums.len() == 4 {
            sh.xs = vec![nums[0], nums[2]];
            sh.ys = vec![nums[1], nums[3]];
            sh.coord = Some((a.first, a.last));
            sh.is_extent = true;
        } else if a.name == "points" && nums.len() >= 2 && nums.len().is_multiple_of(2) {
            sh.xs = nums.iter().step_by(2).copied().collect();
            sh.ys = nums.iter().skip(1).step_by(2).copied().collect();
            sh.coord = Some((a.first, a.last));
            sh.is_extent = false;
        }
    }
    if sh.coord.is_some() {
        for k in 0..sh.xs.len() {
            sh.xs[k] += sh.ox;
            sh.ys[k] += sh.oy;
        }
    }
    sh
}

/// The box (x1, y1, x2, y2) of a shape.
fn shape_box(sh: &Shape) -> FrameBox {
    let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (min(&sh.xs), min(&sh.ys), max(&sh.xs), max(&sh.ys))
}

/// The text of the shape with the coordinates `xs`, `ys` (in the diagram) in place of its own.
fn shape_text(src: &Source, sh: &Shape, coord: (usize, usize), xs: &[f64], ys: &[f64]) -> String {
    let mut s = src.text[src.start(sh.first)..src.start(coord.0)].to_owned();
    if sh.is_extent {
        s.push_str(&extent_text(xs[0] - sh.ox, ys[0] - sh.oy, xs[1] - sh.ox, ys[1] - sh.oy));
    } else {
        s.push_str("points={");
        for k in 0..xs.len() {
            if k > 0 {
                s.push(',');
            }
            s.push_str(&point_text(xs[k] - sh.ox, ys[k] - sh.oy));
        }
        s.push('}');
    }
    s.push_str(&src.text[src.stop(coord.1)..src.stop(sh.last)]);
    s
}

/// True when the point is inside the box, widened by `m`.
fn in_box(b: &FrameBox, x: f64, y: f64, m: f64) -> bool {
    x >= b.0 - m && x <= b.2 + m && y >= b.1 - m && y <= b.3 + m
}

/// Remove the argument `graphics` of `Diagram(...)` (it opens at token `dl`). `{}` is not valid
/// Modelica. When `graphics` is the only argument, it becomes the coordinate system of the
/// diagram, so that the extent that `annotate_extent` writes does not go into an empty list.
fn drop_all_graphics(edits: &mut Vec<Edit>, src: &Source, d: &mut Diagram, dl: usize, g: &Arg) {
    if src.args_of(dl).is_some_and(|(args, _)| args.len() == 1) {
        let e = diagram_extent(d);
        edits.push(Edit {
            start: src.start(g.first),
            stop: src.stop(g.last),
            text: format!("coordinateSystem({})", extent_text(e.0, e.1, e.2, e.3)),
        });
        set_extent(d, e);
    } else {
        src.remove_argument(edits, dl, "graphics");
    }
}

/// The edit that moves or drops the shapes of the Diagram layer of the class, as `opts.graphics`
/// says. Call it once the nodes are placed, before `annotate_extent`.
pub fn annotate_graphics(edits: &mut Vec<Edit>, src: &Source, d: &mut Diagram, opts: &Options) {
    if opts.graphics == "keep" {
        return;
    }
    let diag = find_arg(src, d.ann_lpar, "Diagram");
    let Some(dl) = arg_lpar(src, diag.as_ref()) else {
        return;
    };
    let Some(g) = find_arg(src, Some(dl), "graphics") else {
        return;
    };
    // name = {
    let Some(lbrace) = src.arg_name_token(g.first, g.last).map(|t| t + 2) else {
        return;
    };
    if lbrace > g.last || !matches!(src.kind(lbrace), TokenKind::LBrace) || src.match_close(lbrace) != Some(g.last) {
        return;
    }
    let rbrace = g.last;
    if opts.graphics == "drop" {
        drop_all_graphics(edits, src, d, dl, &g);
        return;
    }
    let Some((items, _)) = src.args_of(lbrace) else {
        return;
    };
    if items.is_empty() {
        return;
    }
    let shapes: Vec<Shape> = items.iter().map(|it| read_shape(src, it)).collect();
    let ns = shapes.len();
    let nodes: Vec<usize> = (0..d.nodes.len()).filter(|i| d.nodes[*i].has_old).collect();
    let nbox = |i: usize| -> FrameBox {
        let n = &d.nodes[i];
        (n.x - n.w / 2.0, n.y - n.h / 2.0, n.x + n.w / 2.0, n.y + n.h / 2.0)
    };
    // Frames: rectangles that hold components of the old layout.
    let mut is_frame = vec![false; ns];
    let mut frame_old: Vec<FrameBox> = vec![(0.0, 0.0, 0.0, 0.0); ns];
    let mut frame_new: Vec<FrameBox> = vec![(0.0, 0.0, 0.0, 0.0); ns];
    let mut dropped = vec![false; ns];
    for (k, sh) in shapes.iter().enumerate() {
        if !(sh.kind == "Rectangle" && sh.is_extent && sh.coord.is_some()) {
            continue;
        }
        let b = shape_box(sh);
        if !(b.2 - b.0 >= MIN_FRAME_SIZE && b.3 - b.1 >= MIN_FRAME_SIZE) {
            continue;
        }
        let members: Vec<usize> = nodes
            .iter()
            .copied()
            .filter(|i| in_box(&b, d.nodes[*i].old_x, d.nodes[*i].old_y, 0.0))
            .collect();
        if members.is_empty() {
            continue;
        }
        let (mut x1, mut y1, mut x2, mut y2) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for i in &members {
            let nb = nbox(*i);
            x1 = x1.min(nb.0);
            y1 = y1.min(nb.1);
            x2 = x2.max(nb.2);
            y2 = y2.max(nb.3);
        }
        let nf = (
            x1 - FRAME_MARGIN,
            y1 - FRAME_MARGIN,
            x2 + FRAME_MARGIN,
            y2 + FRAME_MARGIN,
        );
        is_frame[k] = true;
        frame_old[k] = b;
        frame_new[k] = nf;
        // The frame must not cover a component that is not its own.
        for i in 0..d.nodes.len() {
            if members.contains(&i) {
                continue;
            }
            let nb = nbox(i);
            if nb.0 < nf.2 && nb.2 > nf.0 && nb.1 < nf.3 && nb.3 > nf.1 {
                dropped[k] = true;
                break;
            }
        }
    }
    let mut out: Vec<String> = Vec::new();
    let mut changed = false;
    for (k, sh) in shapes.iter().enumerate() {
        let original = src.token_slice(sh.first, sh.last);
        let Some(coord) = sh.coord else {
            out.push(original.to_owned());
            continue;
        };
        let mut nx = sh.xs.clone();
        let mut ny = sh.ys.clone();
        let b = shape_box(sh);
        let cx = (b.0 + b.2) / 2.0;
        let cy = (b.1 + b.3) / 2.0;
        let mut drop = false;
        if is_frame[k] {
            if dropped[k] {
                drop = true;
            } else {
                nx = vec![frame_new[k].0, frame_new[k].2];
                ny = vec![frame_new[k].1, frame_new[k].3];
            }
        } else {
            // Inside a frame (or a title just above it): the smallest one that holds the centre.
            let mut inside: Option<usize> = None;
            let mut area = f64::INFINITY;
            for f in 0..ns {
                let fo = frame_old[f];
                if !is_frame[f] || !(cx >= fo.0 && cx <= fo.2 && cy >= fo.1 && cy <= fo.3 + FRAME_TITLE) {
                    continue;
                }
                let a = (fo.2 - fo.0) * (fo.3 - fo.1);
                if a < area {
                    area = a;
                    inside = Some(f);
                }
            }
            if let Some(f) = inside {
                if dropped[f] {
                    drop = true;
                } else {
                    let fdx = (frame_new[f].0 + frame_new[f].2) / 2.0 - (frame_old[f].0 + frame_old[f].2) / 2.0;
                    let fdy = frame_new[f].3 - frame_old[f].3;
                    for q in 0..nx.len() {
                        nx[q] += fdx;
                        ny[q] += fdy;
                    }
                }
            } else if (sh.kind == "Rectangle" || sh.kind == "Ellipse")
                && b.2 - b.0 <= MAX_MARKER_SIZE
                && b.3 - b.1 <= MAX_MARKER_SIZE
            {
                drop = true;
            } else if sh.kind == "Rectangle"
                && sh.is_extent
                && b.2 - b.0 >= MIN_FRAME_SIZE
                && b.3 - b.1 >= MIN_FRAME_SIZE
            {
                // A frame around nothing: moved with the nearest component, it would look like that
                // component's frame.
                drop = true;
            } else {
                let mut near: Option<usize> = None;
                let mut best = f64::INFINITY;
                for i in &nodes {
                    let dist = (d.nodes[*i].old_x - cx).abs() + (d.nodes[*i].old_y - cy).abs();
                    if dist < best {
                        best = dist;
                        near = Some(*i);
                    }
                }
                if let Some(i) = near {
                    let dx = d.nodes[i].x - d.nodes[i].old_x;
                    let dy = d.nodes[i].y - d.nodes[i].old_y;
                    for q in 0..nx.len() {
                        nx[q] += dx;
                        ny[q] += dy;
                    }
                }
            }
        }
        if drop {
            changed = true;
            continue;
        }
        // Only a change of the numbers counts: the rest of the text is as it was.
        let same =
            (0..nx.len()).all(|q| (nx[q] - sh.xs[q]).abs() < SAME_POINT && (ny[q] - sh.ys[q]).abs() < SAME_POINT);
        if same {
            out.push(original.to_owned());
        } else {
            out.push(shape_text(src, sh, coord, &nx, &ny));
            changed = true;
        }
    }
    if !changed {
        return;
    }
    if out.is_empty() {
        drop_all_graphics(edits, src, d, dl, &g);
    } else {
        edits.push(Edit {
            start: src.start(lbrace),
            stop: src.stop(rbrace),
            text: format!("{{{}}}", out.join(",\n    ")),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_of_a_text() {
        assert_eq!(scan_numbers("{{-10,2.5},{+3,.5}}"), [-10.0, 2.5, 3.0, 0.5]);
        assert_eq!(scan_numbers("1e2, 2E-1, 3e+1, 4e"), [100.0, 0.2, 30.0, 4.0]);
        assert_eq!(scan_numbers("a - b, 1.2.3, 7"), [7.0]);
    }
}
