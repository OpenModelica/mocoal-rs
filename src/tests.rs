// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The layouts of the models in `test/models`, which use the parts in `test/library`.
//!
//! Each case compares the result with its file in `test/expected`, byte for byte: the
//! `Placement` and `Line` coordinates of every component and connection. After an intended
//! change of the layout, write the files anew and look at the difference:
//!
//! ```text
//! MOCOAL_BLESS=1 cargo test -p mocoal layouts
//! git diff test/expected
//! ```
//!
//! Each case also checks the laid-out diagram of every class: no two components overlap, every
//! line starts and ends at its ports, orthogonal lines are axis-aligned and pass through no
//! component, and a second layout of the result changes nothing. With `keep`, every `Placement`
//! of the input is in the result as it was.

use std::path::PathBuf;

use crate::api::{LayoutResult, layout_diagrams};
use crate::layout::diagram::{Diagram, Node, Point};
use crate::options::Options;
use crate::source::lookup::Library;

/// A model file, the name of its expected file, and the options that differ from the defaults.
struct Case {
    model: &'static str,
    expected: &'static str,
    set: fn(&mut Options),
}

fn defaults(_: &mut Options) {}

const CASES: &[Case] = &[
    Case {
        model: "Chain.mo",
        expected: "Chain.mo",
        set: defaults,
    },
    Case {
        model: "GainChain.mo",
        expected: "GainChain.mo",
        set: defaults,
    },
    Case {
        model: "GainChain.mo",
        expected: "GainChain-down.mo",
        set: |o| o.direction = "down".to_owned(),
    },
    Case {
        model: "Control.mo",
        expected: "Control.mo",
        set: defaults,
    },
    Case {
        model: "RCLadder.mo",
        expected: "RCLadder.mo",
        set: defaults,
    },
    Case {
        model: "ResistorMesh.mo",
        expected: "ResistorMesh.mo",
        set: defaults,
    },
    Case {
        model: "ResistorMesh.mo",
        expected: "ResistorMesh-no-rotate.mo",
        set: |o| o.rotate = false,
    },
    Case {
        model: "ResistorMesh.mo",
        expected: "ResistorMesh-grid-straight.mo",
        set: |o| {
            o.layout = "grid".to_owned();
            o.route = "straight".to_owned();
        },
    },
    Case {
        model: "DriveTrain.mo",
        expected: "DriveTrain.mo",
        set: defaults,
    },
    Case {
        model: "DriveTrain.mo",
        expected: "DriveTrain-spring.mo",
        set: |o| o.layout = "spring".to_owned(),
    },
    Case {
        model: "Features.mo",
        expected: "Features.mo",
        set: defaults,
    },
    Case {
        model: "Features.mo",
        expected: "Features-keep.mo",
        set: |o| o.keep = true,
    },
    Case {
        model: "Tickets.mo",
        expected: "Tickets.mo",
        set: defaults,
    },
    Case {
        model: "Tickets.mo",
        expected: "Tickets-keep-partial.mo",
        set: |o| {
            o.keep = true;
            o.classes = vec!["Tickets.Partial".to_owned()];
        },
    },
];

/// Components that keep a place given elsewhere, so the class does not place them: (class, how many).
const NOT_PLACED: &[(&str, usize)] = &[("Features.Derived", 1)];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test")
}

fn options(case: &Case) -> Options {
    let mut o = Options {
        libraries: vec![root().join("library").to_string_lossy().into_owned()],
        ..Options::default()
    };
    (case.set)(&mut o);
    o
}

/// The layout of `text` and the diagram of each class.
fn layout(case: &Case, text: &str, opts: &Options) -> (LayoutResult, Vec<Diagram>) {
    let mut diagrams = Vec::new();
    let mut lib = Library::new(&opts.libraries);
    let result = layout_diagrams(text, case.model, opts, &mut lib, Some(&mut diagrams))
        .unwrap_or_else(|e| panic!("{}: {e}", case.expected));
    (result, diagrams)
}

#[test]
fn layouts() {
    let bless = std::env::var_os("MOCOAL_BLESS").is_some();
    let mut failed = Vec::new();
    for case in CASES {
        let opts = options(case);
        let source = std::fs::read_to_string(root().join("models").join(case.model)).unwrap();
        let (result, diagrams) = layout(case, &source, &opts);
        let path = root().join("expected").join(case.expected);
        if bless {
            std::fs::write(&path, &result.text).unwrap();
        } else if std::fs::read_to_string(&path).ok().as_deref() != Some(result.text.as_str()) {
            failed.push(format!("{}: differs from {}", case.expected, path.display()));
        }
        let checks = check_report(&result, &opts).and_then(|()| {
            diagrams
                .iter()
                .try_for_each(|d| check_diagram(d, &opts).map_err(|e| format!("{}: {e}", d.class_name)))
        });
        if let Err(e) = checks {
            failed.push(format!("{}: {e}", case.expected));
        }
        if opts.keep
            && let Some(line) = source
                .lines()
                .find(|l| l.contains("Placement(") && !result.text.contains(l))
        {
            failed.push(format!("{}: --keep changes `{}`", case.expected, line.trim()));
        }
        let (again, _) = layout(case, &result.text, &opts);
        if again.changed {
            failed.push(format!("{}: a second layout changes the result", case.expected));
        }
    }
    assert!(failed.is_empty(), "\n{}", failed.join("\n"));
}

/// No warnings; every connection routed, and every component placed (with `keep`: those without a place).
fn check_report(result: &LayoutResult, opts: &Options) -> Result<(), String> {
    if !result.warnings.is_empty() {
        return Err(format!("warnings: {:?}", result.warnings));
    }
    for c in &result.classes {
        let given = NOT_PLACED.iter().find(|(n, _)| *n == c.name).map_or(0, |(_, k)| *k);
        if (!opts.keep && c.placed + given != c.components) || c.routed != c.connections {
            return Err(format!(
                "{}: {} of {} placed, {} of {} routed",
                c.name, c.placed, c.components, c.routed, c.connections
            ));
        }
    }
    Ok(())
}

/// The box of a node: (x1, y1, x2, y2).
fn node_box(n: &Node) -> [f64; 4] {
    [n.x - n.w / 2.0, n.y - n.h / 2.0, n.x + n.w / 2.0, n.y + n.h / 2.0]
}

/// Whether the intervals (a1, a2) and (b1, b2) share more than a point.
fn overlap(a1: f64, a2: f64, b1: f64, b2: f64) -> bool {
    a1.max(b1) < a2.min(b2)
}

/// Whether the segment from `p` to `q` (axis-aligned) passes through the inside of the box `b`.
fn crosses(p: Point, q: Point, b: [f64; 4]) -> bool {
    let (x1, x2) = (p.0.min(q.0), p.0.max(q.0));
    let (y1, y2) = (p.1.min(q.1), p.1.max(q.1));
    let inside_x = |x: f64| b[0] < x && x < b[2];
    let inside_y = |y: f64| b[1] < y && y < b[3];
    if x1 == x2 {
        inside_x(x1) && overlap(y1, y2, b[1], b[3])
    } else {
        inside_y(y1) && overlap(x1, x2, b[0], b[2])
    }
}

fn same(p: Point, q: Point) -> bool {
    (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9
}

fn check_diagram(d: &Diagram, opts: &Options) -> Result<(), String> {
    for (i, a) in d.nodes.iter().enumerate() {
        for b in &d.nodes[i + 1..] {
            let (p, q) = (node_box(a), node_box(b));
            if overlap(p[0], p[2], q[0], q[2]) && overlap(p[1], p[3], q[1], q[3]) {
                return Err(format!("{} and {} overlap", a.name, b.name));
            }
        }
    }
    for l in &d.links {
        let (a, b) = (&d.nodes[l.a], &d.nodes[l.b]);
        let name = format!("connect({}, {})", a.name, b.name);
        let (Some(&first), Some(&last)) = (l.points.first(), l.points.last()) else {
            return Err(format!("{name} has no line"));
        };
        let (pa, pb) = (a.port_position(l.pa), b.port_position(l.pb));
        if !(same(first, pa) && same(last, pb) || same(first, pb) && same(last, pa)) {
            return Err(format!("{name} does not end at its ports: {:?}", l.points));
        }
        if opts.route != "orthogonal" {
            continue;
        }
        let segments = l.points.len() - 1;
        for (k, w) in l.points.windows(2).enumerate() {
            let (p, q) = (w[0], w[1]);
            if p.0 != q.0 && p.1 != q.1 {
                return Err(format!("{name} is not axis-aligned: {:?}", l.points));
            }
            // A port may lie inside its own component: the first and last segments leave it.
            for (j, n) in d.nodes.iter().enumerate() {
                let own = (k == 0 || k + 1 == segments) && (j == l.a || j == l.b);
                if !own && crosses(p, q, node_box(n)) {
                    return Err(format!("{name} passes through {}: {:?}", n.name, l.points));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn checks_find_faults() {
    use crate::layout::diagram::Link;
    let opts = Options::default();
    let mut d = Diagram::new("Faults");
    for (name, x) in [("a", 0.0), ("b", 15.0), ("c", 60.0)] {
        let mut n = Node::new(name, "T", 20.0, 20.0);
        n.x = x;
        d.nodes.push(n);
    }
    assert!(check_diagram(&d, &opts).unwrap_err().contains("a and b overlap"));
    d.nodes[1].y = 40.0;
    assert!(check_diagram(&d, &opts).is_ok());
    let mut l = Link::new(0, None, 1, None);
    l.points = vec![(0.0, 0.0), (15.0, 40.0)];
    d.links.push(l);
    assert!(check_diagram(&d, &opts).unwrap_err().contains("not axis-aligned"));
    d.links[0].points = vec![(0.0, 0.0), (0.0, 40.0), (15.0, 40.0)];
    assert!(check_diagram(&d, &opts).is_ok());
    d.links[0].points = vec![(0.0, 0.0), (0.0, 30.0)];
    assert!(
        check_diagram(&d, &opts)
            .unwrap_err()
            .contains("does not end at its ports")
    );
    d.links[0] = Link::new(0, None, 2, None);
    d.links[0].points = vec![(0.0, 0.0), (60.0, 0.0)];
    d.nodes[1].y = 0.0;
    d.nodes[1].x = 30.0;
    assert!(check_diagram(&d, &opts).unwrap_err().contains("passes through b"));
}
