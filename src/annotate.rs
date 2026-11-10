// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Writing a diagram back into the source: `Placement` for the components, `Line` for the connect
//! equations, and the diagram's extent in the class annotation. Every change is an `Edit` of the
//! text, so the rest of the source stays as it is.
//! Mirrors `src/annotate.jl` of ModelicaComponentAutolayout.jl.

use openmodelica_ast::parser::lexer::TokenKind;

use crate::absyn::format_number;
use crate::diagram::{Diagram, Link, Node};
use crate::tokens::{Arg, Edit, Source};

pub fn point_text(x: f64, y: f64) -> String {
    format!("{{{},{}}}", format_number(x), format_number(y))
}

pub fn extent_text(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    format!("extent={{{},{}}}", point_text(x1, y1), point_text(x2, y2))
}

/// `transformation(origin={x,y}, extent={{..},{..}})` of a node placed by the layout.
pub fn transformation_text(n: &Node) -> String {
    let ew = n.extent_width();
    let eh = n.extent_height();
    let rot = if n.rot == 0 {
        String::new()
    } else {
        format!(", rotation={}", n.rot)
    };
    format!(
        "transformation(origin={}, {}{})",
        point_text(n.x, n.y),
        extent_text(-ew / 2.0, -eh / 2.0, ew / 2.0, eh / 2.0),
        rot
    )
}

fn is_lpar(src: &Source, t: usize) -> bool {
    t < src.token_count() && matches!(src.kind(t), TokenKind::LParen)
}

/// The argument `name` of the list that opens at token `lpar`.
pub fn find_arg(src: &Source, lpar: Option<usize>, name: &str) -> Option<Arg> {
    let lpar = lpar.filter(|t| is_lpar(src, *t))?;
    let (args, _) = src.args_of(lpar)?;
    args.into_iter().find(|a| a.name == name)
}

/// The `(` that opens the argument list of the argument `a`.
pub fn arg_lpar(src: &Source, a: Option<&Arg>) -> Option<usize> {
    let a = a?;
    let t = src.arg_name_token(a.first, a.last)?;
    (t < a.last && is_lpar(src, t + 1)).then_some(t + 1)
}

/// The text between the brackets of the argument `a`, "" when it has none.
pub fn arg_inner(src: &Source, a: Option<&Arg>) -> String {
    let Some(lpar) = arg_lpar(src, a) else {
        return String::new();
    };
    match src.match_close(lpar) {
        Some(rpar) if rpar > lpar + 1 => src.token_slice(lpar + 1, rpar - 1).to_owned(),
        _ => String::new(),
    }
}

/// The `iconTransformation` of the class's own connectors that have no Placement yet, by node
/// ("" for none): the places of `icon_places`, which the port lookup uses too, so a second run
/// sees the same ports.
fn icon_transformations(d: &Diagram) -> Vec<String> {
    let mut out = vec![String::new(); d.nodes.len()];
    for (i, n) in d.nodes.iter().enumerate() {
        if !(n.is_connector && !n.fixed && !n.inherited && !n.has_transformation && !n.has_icon_transformation) {
            continue;
        }
        if let Some(p) = d.icon_places.get(&n.name) {
            out[i] = format!(
                "iconTransformation(origin={}, {})",
                point_text(p.0, p.1),
                extent_text(-p.2 / 2.0, -p.3 / 2.0, p.2 / 2.0, p.3 / 2.0)
            );
        }
    }
    out
}

fn insertion(at: usize, text: String) -> Edit {
    Edit {
        start: at,
        stop: at,
        text,
    }
}

/// The edits that give the free nodes their Placement. Returns the number of nodes placed.
pub fn annotate_nodes(edits: &mut Vec<Edit>, src: &Source, d: &Diagram, warnings: &mut Vec<String>) -> usize {
    let icons = icon_transformations(d);
    let mut count = 0;
    for (i, n) in d.nodes.iter().enumerate() {
        if n.fixed || n.inherited {
            continue;
        }
        let Some(seg) = n.seg else {
            warnings.push(format!(
                "{}: cannot locate {} in the source; not placed",
                d.class_name, n.name
            ));
            continue;
        };
        let tr = transformation_text(n);
        let icon = &icons[i];
        if let Some(lpar) = n.target.map(|t| t + 1).filter(|t| is_lpar(src, *t)) {
            let placement = find_arg(src, Some(lpar), "Placement");
            match arg_lpar(src, placement.as_ref()) {
                None => {
                    let leaf = if icon.is_empty() {
                        format!("Placement({tr})")
                    } else {
                        format!("Placement({tr}, {icon})")
                    };
                    src.set_modifier(edits, lpar, &["Placement"], 0, &leaf);
                }
                Some(plpar) => {
                    // The connector's icon keeps its place: the old transformation becomes the
                    // iconTransformation.
                    let old = find_arg(src, Some(plpar), "transformation");
                    let has_icon = find_arg(src, Some(plpar), "iconTransformation").is_some();
                    src.set_modifier(edits, plpar, &["transformation"], 0, &tr);
                    if n.is_connector && !has_icon {
                        let inner = arg_inner(src, old.as_ref());
                        if !inner.is_empty() {
                            let leaf = format!("iconTransformation({inner})");
                            src.set_modifier(edits, plpar, &["iconTransformation"], 0, &leaf);
                        } else if !icon.is_empty() {
                            src.set_modifier(edits, plpar, &["iconTransformation"], 0, icon);
                        }
                    }
                }
            }
        } else {
            let text = if icon.is_empty() {
                format!(" annotation(Placement({tr}))")
            } else {
                format!(" annotation(Placement({tr}, {icon}))")
            };
            edits.push(insertion(src.stop(seg.delim - 1), text));
        }
        count += 1;
    }
    count
}

/// `Line(points={...}, ...)` of a link; `kept` are the arguments of the Line it replaces.
pub fn line_text(l: &Link, kept: &[String], has_color: bool) -> String {
    let mut s = String::from("Line(points={");
    for (k, p) in l.points.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        s.push_str(&point_text(p.0, p.1));
    }
    s.push('}');
    for a in kept {
        s.push_str(", ");
        s.push_str(a);
    }
    if !has_color && !l.color.is_empty() {
        s.push_str(", color=");
        s.push_str(&l.color);
    }
    if l.smooth {
        s.push_str(", smooth=Smooth.Bezier");
    }
    s.push(')');
    s
}

/// The edits that give the links their Line. Returns the number of lines written.
pub fn annotate_links(edits: &mut Vec<Edit>, src: &Source, d: &Diagram) -> usize {
    let mut count = 0;
    for l in &d.links {
        let (false, Some(_), Some(semi_tok)) = (l.keep, l.eq_tok, l.semi_tok) else {
            continue;
        };
        let lpar = l.ann_tok.map(|t| t + 1).filter(|t| is_lpar(src, *t));
        let line = find_arg(src, lpar, "Line");
        if l.points.len() < 2 {
            // No line: one that exists no longer fits the placement.
            if let (Some(_), Some(lpar), Some(ann_tok)) = (&line, lpar, l.ann_tok)
                && let Some((args, rpar)) = src.args_of(lpar)
            {
                if args.len() == 1 {
                    edits.push(Edit {
                        start: src.stop(ann_tok - 1),
                        stop: src.stop(rpar),
                        text: String::new(),
                    });
                } else {
                    src.remove_argument(edits, lpar, "Line");
                }
            }
            continue;
        }
        let mut kept: Vec<String> = Vec::new();
        let mut has_color = false;
        if let Some(llpar) = arg_lpar(src, line.as_ref())
            && let Some((largs, _)) = src.args_of(llpar)
        {
            for a in &largs {
                if matches!(a.name.as_str(), "points" | "smooth" | "origin" | "rotation") {
                    continue;
                }
                if a.name == "color" {
                    has_color = true;
                }
                kept.push(src.token_slice(a.first, a.last).to_owned());
            }
        }
        let text = line_text(l, &kept, has_color);
        match lpar {
            Some(lpar) => {
                src.set_modifier(edits, lpar, &["Line"], 0, &text);
            }
            None => edits.push(insertion(src.stop(semi_tok - 1), format!(" annotation({text})"))),
        }
        count += 1;
    }
    count
}

/// The extent the diagram needs: the one it has (±100 by default), extended to hold the layout
/// with a margin of 20, at multiples of 20.
pub fn diagram_extent(d: &Diagram) -> (f64, f64, f64, f64) {
    let b = d.bounding_box();
    (
        d.x1.min(((b.0 - 20.0) / 20.0).floor() * 20.0),
        d.y1.min(((b.1 - 20.0) / 20.0).floor() * 20.0),
        d.x2.max(((b.2 + 20.0) / 20.0).ceil() * 20.0),
        d.y2.max(((b.3 + 20.0) / 20.0).ceil() * 20.0),
    )
}

/// Take `e` as the extent of the diagram.
pub fn set_extent(d: &mut Diagram, e: (f64, f64, f64, f64)) {
    (d.x1, d.y1, d.x2, d.y2) = e;
    d.has_extent = true;
}

/// The edit that extends the diagram's coordinate system when the layout does not fit in it.
pub fn annotate_extent(edits: &mut Vec<Edit>, src: &Source, d: &mut Diagram, warnings: &mut Vec<String>) -> bool {
    let e = diagram_extent(d);
    if e == (d.x1, d.y1, d.x2, d.y2) {
        return false;
    }
    let leaf = extent_text(e.0, e.1, e.2, e.3);
    if let Some(lpar) = d.ann_lpar {
        if !src.set_modifier(edits, lpar, &["Diagram", "coordinateSystem", "extent"], 0, &leaf) {
            return false;
        }
    } else if let Some(end_tok) = d.end_tok {
        let at = src.start(end_tok);
        let bytes = src.text.as_bytes();
        let mut line_first = at;
        while line_first > 0 && matches!(bytes[line_first - 1], b' ' | b'\t') {
            line_first -= 1;
        }
        let text = format!("annotation(Diagram(coordinateSystem({leaf})));");
        if line_first == 0 || bytes[line_first - 1] == b'\n' {
            let indent = &src.text[line_first..at];
            edits.push(insertion(at, format!("  {text}\n{indent}")));
        } else {
            edits.push(insertion(at, format!("{text} ")));
        }
    } else {
        warnings.push(format!(
            "{}: cannot locate the end of the class; the diagram extent is not written",
            d.class_name
        ));
        return false;
    }
    set_extent(d, e);
    true
}
