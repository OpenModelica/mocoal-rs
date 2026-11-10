// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! An SVG preview of a diagram: boxes with names for the components, dots for their connectors,
//! and the connection lines. It shows the layout, not the icons.
//! Mirrors `src/svg.jl` of ModelicaComponentAutolayout.jl.

use crate::absyn::format_number;
use crate::diagram::Diagram;

/// `{r,g,b}` as an SVG colour, or `default`.
pub fn svg_color(c: &str, default: &str) -> String {
    if !(c.len() >= 7 && c.starts_with('{') && c.ends_with('}')) {
        return default.to_owned();
    }
    format!("rgb({})", &c[1..c.len() - 1])
}

pub fn svg_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The SVG preview of a placed and routed diagram.
pub fn svg_diagram(d: &Diagram) -> String {
    let bx = d.bounding_box();
    let x1 = bx.0.min(d.x1) - 20.0;
    let y1 = bx.1.min(d.y1) - 20.0;
    let x2 = bx.2.max(d.x2) + 20.0;
    let y2 = bx.3.max(d.y2) + 20.0;
    let f = 3.0;
    let mut io = String::new();
    let n = |v: f64| format_number(v);
    let px = |x: f64| format_number((x - x1) * f);
    let py = |y: f64| format_number((y2 - y) * f);
    io += &format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" font-family=\"Helvetica, Arial, sans-serif\">\n",
        n((x2 - x1) * f),
        n((y2 - y1) * f),
        n((x2 - x1) * f),
        n((y2 - y1) * f)
    );
    io += &format!("<title>{}</title>\n", svg_escape(&d.class_name));
    io += "<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n";
    io += &format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#bbbbbb\" stroke-dasharray=\"6 4\"/>\n",
        px(d.x1),
        py(d.y2),
        n((d.x2 - d.x1) * f),
        n((d.y2 - d.y1) * f)
    );
    for l in &d.links {
        if l.points.len() < 2 {
            continue;
        }
        let color = svg_color(&l.color, "#444444");
        if l.smooth && l.points.len() == 4 {
            io += &format!(
                "<path fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\" d=\"M{} {} C",
                color,
                px(l.points[0].0),
                py(l.points[0].1)
            );
            for p in &l.points[1..4] {
                io += &format!(" {} {}", px(p.0), py(p.1));
            }
            io += "\"/>\n";
        } else {
            io += &format!(
                "<polyline fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\" points=\"",
                color
            );
            for p in &l.points {
                io += &format!("{},{} ", px(p.0), py(p.1));
            }
            io += "\"/>\n";
        }
    }
    for node in &d.nodes {
        let stroke = if node.fixed { "#999999" } else { "#222222" };
        let fill = if node.is_connector {
            svg_color(&node.color, "#888888")
        } else {
            "#f2f2f2".to_owned()
        };
        io += &format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{} stroke=\"{}\"/>\n",
            px(node.x - node.w / 2.0),
            py(node.y + node.h / 2.0),
            n(node.w * f),
            n(node.h * f),
            fill,
            if node.is_connector {
                " fill-opacity=\"0.35\""
            } else {
                ""
            },
            stroke
        );
        for (pi, port) in node.ports.iter().enumerate() {
            if !port.known {
                continue;
            }
            let q = node.port_position(Some(pi));
            io += &format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"3\" fill=\"{}\"/>\n",
                px(q.0),
                py(q.1),
                svg_color(&port.color, "#444444")
            );
        }
        io += &format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"11\" text-anchor=\"middle\" fill=\"#222222\">{}</text>\n",
            px(node.x),
            format_number((y2 - node.y - node.h / 2.0) * f - 4.0),
            svg_escape(&node.name)
        );
    }
    io += "</svg>\n";
    io
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{Link, Node, Port};

    fn link(points: Vec<(f64, f64)>, color: &str, smooth: bool) -> Link {
        Link {
            a: 0,
            pa: None,
            b: 0,
            pb: None,
            points,
            eq_tok: None,
            semi_tok: None,
            ann_tok: None,
            has_line: false,
            color: color.to_owned(),
            smooth,
            keep: false,
        }
    }

    #[test]
    fn colours_and_text() {
        assert_eq!(svg_color("{0,0,127}", "#444444"), "rgb(0,0,127)");
        assert_eq!(svg_color("{0,0,0}", "#444444"), "rgb(0,0,0)");
        assert_eq!(svg_color("{0,0}", "#444444"), "#444444");
        assert_eq!(svg_color("", "#888888"), "#888888");
        assert_eq!(svg_color("0,0,127}", "x"), "x");
        assert_eq!(svg_escape("a<b> & &lt;"), "a&lt;b&gt; &amp; &amp;lt;");
    }

    #[test]
    fn empty_diagram() {
        // The class's extent of ±100 with a margin of 20, 3 pixels per unit.
        let expected = concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"720\" height=\"720\" viewBox=\"0 0 720 720\" ",
            "font-family=\"Helvetica, Arial, sans-serif\">\n",
            "<title>M&lt;1&gt;</title>\n",
            "<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n",
            "<rect x=\"60\" y=\"60\" width=\"600\" height=\"600\" fill=\"none\" stroke=\"#bbbbbb\" stroke-dasharray=\"6 4\"/>\n",
            "</svg>\n"
        );
        assert_eq!(svg_diagram(&Diagram::new("M<1>")), expected);
    }

    #[test]
    fn nodes_and_lines() {
        let mut d = Diagram::new("M");
        let mut a = Node::new("a", "T", 20.0, 20.0);
        a.placed = true;
        a.set_free();
        for (name, ix, known, color) in [("u", -120.0, false, ""), ("y", 110.0, true, "{0,0,127}")] {
            a.add_port(Port {
                name: name.to_owned(),
                ix,
                iy: 0.0,
                known,
                causality: 0,
                color: color.to_owned(),
            });
        }
        let mut c = Node::new("c&d", "T", 10.0, 30.5);
        c.x = 105.0;
        c.y = -50.0;
        c.fixed = true;
        c.is_connector = true;
        d.nodes = vec![a, c];
        d.links = vec![
            link(vec![(0.0, 0.0), (10.0, 5.25)], "", false),
            link(vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 1.0)], "{255,0,0}", true),
            link(vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], "", true),
            link(vec![(0.0, 0.0)], "", false),
        ];
        // x from -120 to 130 (node c reaches 110), y from -120 to 120.
        let expected = concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"750\" height=\"720\" viewBox=\"0 0 750 720\" ",
            "font-family=\"Helvetica, Arial, sans-serif\">\n",
            "<title>M</title>\n",
            "<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n",
            "<rect x=\"60\" y=\"60\" width=\"600\" height=\"600\" fill=\"none\" stroke=\"#bbbbbb\" stroke-dasharray=\"6 4\"/>\n",
            "<polyline fill=\"none\" stroke=\"#444444\" stroke-width=\"1.5\" points=\"360,360 390,344.25 \"/>\n",
            "<path fill=\"none\" stroke=\"rgb(255,0,0)\" stroke-width=\"1.5\" d=\"M360 360 C 363 360 366 360 369 357\"/>\n",
            "<polyline fill=\"none\" stroke=\"#444444\" stroke-width=\"1.5\" points=\"360,360 363,360 366,360 \"/>\n",
            "<rect x=\"330\" y=\"330\" width=\"60\" height=\"60\" fill=\"#f2f2f2\" stroke=\"#222222\"/>\n",
            "<circle cx=\"393\" cy=\"360\" r=\"3\" fill=\"rgb(0,0,127)\"/>\n",
            "<text x=\"360\" y=\"326\" font-size=\"11\" text-anchor=\"middle\" fill=\"#222222\">a</text>\n",
            "<rect x=\"660\" y=\"464.25\" width=\"30\" height=\"91.5\" fill=\"#888888\" fill-opacity=\"0.35\" stroke=\"#999999\"/>\n",
            "<text x=\"675\" y=\"460.25\" font-size=\"11\" text-anchor=\"middle\" fill=\"#222222\">c&amp;d</text>\n",
            "</svg>\n"
        );
        assert_eq!(svg_diagram(&d), expected);
    }
}
