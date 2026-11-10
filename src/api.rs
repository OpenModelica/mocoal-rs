// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The layout of the classes of a source text: parse, extract the diagrams, place, route, and
//! write the annotations back.
//! Mirrors `src/api.jl` of ModelicaComponentAutolayout.jl.

use std::rc::Rc;

use crate::annotate::{annotate_extent, annotate_links, annotate_nodes};
use crate::extract::{class_selected, diagram_classes, extract_diagram};
use crate::graphics::annotate_graphics;
use crate::lookup::{Library, file_path, parse_text};
use crate::options::Options;
use crate::placement::place_diagram;
use crate::resolve::register_program;
use crate::routing::route_diagram;
use crate::svg::svg_diagram;
use crate::tokens::{Edit, Source, apply_edits};

/// The version of the implementation.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What was done to one class: its components and connections, and how many got a new
/// annotation.
#[derive(Clone, Debug)]
pub struct ClassReport {
    pub name: String,
    pub components: usize,
    pub connections: usize,
    pub placed: usize,
    pub routed: usize,
}

/// The result of a layout.
#[derive(Clone, Debug)]
pub struct LayoutResult {
    /// The source with the new annotations (the input when nothing changed, or on an error).
    pub text: String,
    /// Whether `text` differs from the input.
    pub changed: bool,
    /// A report per class with a diagram.
    pub classes: Vec<ClassReport>,
    /// What the layout had to do without.
    pub warnings: Vec<String>,
    /// Why there is no layout, or "".
    pub error: String,
    /// The class names and the SVG previews (with `Options::svg`).
    pub svg_names: Vec<String>,
    pub svgs: Vec<String>,
}

impl LayoutResult {
    fn new(text: &str) -> LayoutResult {
        LayoutResult {
            text: text.to_owned(),
            changed: false,
            classes: Vec::new(),
            warnings: Vec::new(),
            error: String::new(),
            svg_names: Vec::new(),
            svgs: Vec::new(),
        }
    }

    fn failed(text: &str, error: String) -> LayoutResult {
        let mut res = LayoutResult::new(text);
        res.error = error;
        res
    }
}

/// `layout_string` with a library that is shared between the files of a run.
pub fn layout_string_in(text: &str, file_name: &str, opts: &Options, lib: &mut Library) -> LayoutResult {
    if let Err(msg) = opts.check() {
        return LayoutResult::failed(text, msg);
    }
    let prog = match parse_text(text, file_name) {
        Ok(p) => p,
        Err(msg) => return LayoutResult::failed(text, msg),
    };
    let src = match Source::new(text, file_name) {
        Ok(s) => Rc::new(s),
        Err(msg) => return LayoutResult::failed(text, msg),
    };
    let mut res = LayoutResult::new(text);
    let known = lib.warnings.len();
    let refs = register_program(lib, &prog, file_name);
    let key = match refs.first() {
        Some(r) => file_path(&lib.classes[*r].file),
        None => file_path(file_name),
    };
    lib.targets.insert(key.clone());
    lib.sources.insert(key.clone(), Some(src.clone()));
    let mut edits: Vec<Edit> = Vec::new();
    let mut matched = false;
    for r in &refs {
        let mut classes = Vec::new();
        diagram_classes(&mut classes, lib, *r, 0);
        for c in classes {
            let name = lib.classes[c].name.clone();
            if !class_selected(&name, &opts.classes) {
                continue;
            }
            matched = true;
            let Some(mut d) = extract_diagram(lib, &src, c, opts, &mut res.warnings) else {
                continue;
            };
            place_diagram(&mut d, opts);
            route_diagram(&mut d, opts);
            let placed = annotate_nodes(&mut edits, &src, &d, &mut res.warnings);
            let routed = annotate_links(&mut edits, &src, &d);
            if placed > 0 || routed > 0 {
                annotate_graphics(&mut edits, &src, &mut d, opts);
                annotate_extent(&mut edits, &src, &mut d, &mut res.warnings);
            }
            res.classes.push(ClassReport {
                name: name.clone(),
                components: d.nodes.len(),
                connections: d.links.len(),
                placed,
                routed,
            });
            if opts.svg {
                res.svg_names.push(name);
                res.svgs.push(svg_diagram(&d));
            }
        }
    }
    lib.sources.remove(&key);
    res.warnings.extend_from_slice(&lib.warnings[known..]);
    if !matched && !opts.classes.is_empty() {
        res.warnings
            .push(format!("{}: no class matches {}", file_name, opts.classes.join(", ")));
    }
    let (out, skipped) = apply_edits(text, &edits);
    if skipped > 0 {
        res.warnings.push(format!(
            "{file_name}: {skipped} annotations overlap others and are not written"
        ));
    }
    if out == text {
        return res;
    }
    if let Err(msg) = parse_text(&out, file_name) {
        res.error = format!("internal error, the result does not parse ({msg}); nothing is changed");
        return res;
    }
    res.text = out;
    res.changed = true;
    res
}

/// Lay out the diagrams of the classes in the Modelica source `text`. `file_name` is the file
/// the text comes from ("" for none): with a `within` clause it tells where the enclosing
/// package is on disk.
pub fn layout_string(text: &str, file_name: &str, opts: &Options) -> LayoutResult {
    layout_string_in(text, file_name, opts, &mut Library::new(&opts.libraries))
}

/// The contents of a file. The layout writes the text back, so it has to be UTF-8 as it is.
pub fn read_source(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|_| format!("{path}: cannot read"))?;
    String::from_utf8(bytes).map_err(|_| format!("{path}: not UTF-8"))
}

/// Write `text` to the file `path`. False when that fails.
pub fn write_text(path: &str, text: &str) -> bool {
    std::fs::write(path, text).is_ok()
}

/// `layout_file` with a library that is shared between the files of a run.
pub fn layout_file_in(path: &str, opts: &Options, lib: &mut Library) -> LayoutResult {
    match read_source(path) {
        Ok(text) => layout_string_in(&text, path, opts, lib),
        Err(msg) => LayoutResult::failed("", msg),
    }
}

/// Lay out the diagrams of the classes in the file `path`. The file is not changed: write the
/// `text` of the result where it should go.
pub fn layout_file(path: &str, opts: &Options) -> LayoutResult {
    layout_file_in(path, opts, &mut Library::new(&opts.libraries))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAIN: &str = "model Chain
  block Gain
    input Real u;
    output Real y;
  end Gain;
  Gain a, b;
  Gain c;
equation
  connect(a.y, b.u);
  connect(b.y, c.u);
end Chain;
";

    #[test]
    fn layout_of_a_chain() {
        let res = layout_string(CHAIN, "", &Options::default());
        assert_eq!(res.error, "");
        assert!(res.changed);
        assert_eq!(res.text.matches("Placement(transformation(origin=").count(), 3);
        assert_eq!(res.text.matches("Line(points=").count(), 2);
        assert_eq!(res.classes.len(), 1);
        assert_eq!((res.classes[0].placed, res.classes[0].routed), (3, 2));
        // The second layout changes nothing.
        let again = layout_string(&res.text, "", &Options::default());
        assert_eq!(again.error, "");
        assert_eq!(again.text, res.text);
        // Existing placements are kept with `keep`.
        let keep = Options {
            keep: true,
            ..Options::default()
        };
        assert!(!layout_string(&res.text, "", &keep).changed);
    }

    #[test]
    fn no_layout() {
        let res = layout_string("model M Real x; end N;", "M.mo", &Options::default());
        assert!(res.error.starts_with("M.mo:"), "{}", res.error);
        assert!(!res.changed);
        let none = layout_string("", "", &Options::default());
        assert_eq!((none.error.as_str(), none.changed, none.text.as_str()), ("", false, ""));
        let other = Options {
            classes: vec!["Other".to_owned()],
            ..Options::default()
        };
        let res = layout_string(CHAIN, "Chain.mo", &other);
        assert_eq!(res.warnings, ["Chain.mo: no class matches Other"]);
    }
}
