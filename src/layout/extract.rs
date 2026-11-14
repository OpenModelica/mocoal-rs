// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the OSMC Public License (OSMC-PL) version 1.8 or
// the GNU AGPL version 3 license, according to the recipient's choice. See LICENSE.md.

//! From a parsed class to its diagram: nodes from the component declarations, links from the
//! connect equations, and the tokens at which the annotations are written back.
//! Mirrors `src/extract.jl` of ModelicaComponentAutolayout.jl.

use std::rc::Rc;

use metamodelica::{List, Ref};
use openmodelica_ast::Absyn;
use openmodelica_ast::parser::lexer::TokenKind;

use crate::layout::diagram::{ACAUSAL, Diagram, Link, Node, Port};
use crate::layout::placement::place_diagram;
use crate::options::Options;
use crate::source::absyn::{
    Transformation, class_annotation_args, comment_args, find_modifier, find_modifier_path, info_position,
    modifier_args, modifier_numbers, path_parts, type_path,
};
use crate::source::lookup::{ClassId, DEFAULT_ICON_SYSTEM, LayoutPlaces, Library, file_path, read_text};
use crate::source::resolve::{
    ClassKind, MAX_LOOKUP_DEPTH, base_classes, class_color, direction_code, find_component, icon_places, icon_system,
    is_builtin_path, kind_and_causality, lookup_path, port_info, read_placement, restriction_kind,
    transformation_centre,
};
use crate::source::tokens::Source;

struct Extraction<'a> {
    lib: &'a mut Library,
    src: &'a Source,
    class: ClassId,
    d: Diagram,
    /// The class of every node, none when it did not resolve.
    refs: Vec<Option<ClassId>>,
    opts: &'a Options,
    warnings: &'a mut Vec<String>,
}

/// The token at the start position of `info`.
pub fn start_token(src: &Source, info: &Absyn::Info) -> Option<usize> {
    let (line, col, _, _) = info_position(info);
    let mut t = src.token_at(line, col);
    if t.is_none() && line == 1 {
        t = src.token_at(line, col + 1).or_else(|| src.token_at(line, col - 1));
    }
    t
}

/// The `end` token of the `end Name` of a class.
pub fn class_end_token(src: &Source, cls: &Absyn::Class) -> Option<usize> {
    let (_, _, line, col) = info_position(&cls.info);
    // The end position is at the token after `end Name`, or past the last token of the text.
    let i = src.token_before(line, col + 1)?;
    for j in (i.saturating_sub(5).max(1)..=i).rev() {
        if matches!(src.kind(j), TokenKind::Ident(_))
            && matches!(src.kind(j - 1), TokenKind::End)
            && src.token_text(j) == cls.name.as_str()
        {
            return Some(j - 1);
        }
    }
    None
}

/// The `(` of the first class annotation, found from its first argument.
pub fn class_annotation_lpar(src: &Source, cls: &Absyn::Class) -> Option<usize> {
    let args = class_annotation_args(cls);
    let t = match &**args.first()? {
        Absyn::MODIFICATION { info, .. } | Absyn::REDECLARATION { info, .. } => start_token(src, info)?,
        _ => return None,
    };
    for j in (t.saturating_sub(4).max(1)..t).rev() {
        if matches!(src.kind(j), TokenKind::LParen) && matches!(src.kind(j - 1), TokenKind::Annotation) {
            return Some(j);
        }
    }
    None
}

/// A parameter or a constant.
pub fn is_parameter(a: &Absyn::ElementAttributes) -> bool {
    matches!(a.variability, Absyn::Variability::PARAM | Absyn::Variability::CONST)
}

/// The first two identifiers of a component reference.
pub fn connect_end(c: &Absyn::ComponentRef) -> (String, String) {
    let mut r = c;
    if let Absyn::CREF_FULLYQUALIFIED { componentRef: inner } = r {
        r = inner;
    }
    match r {
        Absyn::CREF_QUAL {
            name,
            componentRef: rest,
            ..
        } => match &**rest {
            Absyn::CREF_QUAL { name: second, .. } | Absyn::CREF_IDENT { name: second, .. } => {
                (name.to_string(), second.to_string())
            }
            _ => (name.to_string(), String::new()),
        },
        Absyn::CREF_IDENT { name, .. } => (name.to_string(), String::new()),
        _ => (String::new(), String::new()),
    }
}

impl Extraction<'_> {
    /// Create the node of a declaration of class `tref` (none when unresolved).
    fn make_node(
        &mut self,
        name: &str,
        type_name: &str,
        tref: Option<ClassId>,
        kind: Option<ClassKind>,
        direction: i32,
        t: &Transformation,
    ) -> Node {
        let icon = match tref {
            Some(c) => icon_system(self.lib, c),
            None => DEFAULT_ICON_SYSTEM,
        };
        let (w, h) = if t.has {
            ((t.ex2 - t.ex1).abs().max(2.0), (t.ey2 - t.ey1).abs().max(2.0))
        } else {
            icon.default_size(self.opts.scale)
        };
        let mut n = Node::new(name, type_name, w, h);
        n.cx1 = icon.x1;
        n.cy1 = icon.y1;
        n.cx2 = icon.x2;
        n.cy2 = icon.y2;
        n.preserve_aspect_ratio = icon.preserve_aspect_ratio;
        n.is_connector = kind.is_some_and(ClassKind::is_connector);
        n.causality = direction;
        if n.is_connector
            && let Some(c) = tref
        {
            n.color = class_color(self.lib, c, 0);
        }
        n
    }

    fn add_node(&mut self, n: Node, tref: Option<ClassId>) -> usize {
        let i = self.d.nodes.len();
        self.d.index.insert(n.name.clone(), i);
        self.d.nodes.push(n);
        self.refs.push(tref);
        i
    }

    /// The nodes of one component clause.
    fn extract_element(&mut self, e: &Absyn::Element) {
        let Absyn::ELEMENT {
            innerOuter: inner_outer,
            specification,
            info,
            constrainClass: constrain_class,
            ..
        } = e
        else {
            return;
        };
        let Absyn::COMPONENTS {
            attributes,
            typeSpec: type_spec,
            components,
        } = &**specification
        else {
            return;
        };
        if matches!(inner_outer, Absyn::InnerOuter::OUTER) {
            return;
        }
        let tp = type_path(type_spec);
        let parts = path_parts(tp);
        if is_builtin_path(&parts) {
            return;
        }
        let type_name = parts.join(".");
        let tref = lookup_path(self.lib, self.class, tp, false, 0);
        let (kind, direction) = kind_and_causality(self.lib, tref, direction_code(attributes));
        if kind.is_some_and(ClassKind::is_not_drawn) {
            return;
        }
        let items: Vec<&Ref<Absyn::ComponentItem>> = components.iter().collect();
        let segs = match start_token(self.src, info) {
            Some(s) => self.src.element_segments(s),
            None => Vec::new(),
        };
        if segs.len() != items.len() {
            self.warnings.push(format!(
                "{}: cannot locate the declaration of {} in the source; skipped",
                self.d.class_name, type_name
            ));
            return;
        }
        let cc_args = match constrain_class {
            Some(cc) => comment_args(cc.comment.as_ref()),
            None => Vec::new(),
        };
        for (i, item) in items.iter().enumerate() {
            let seg = segs[i];
            let (mut has, mut t, mut it) = read_placement(&comment_args(item.comment.as_ref()));
            let mut target = seg.ann_tok.or(seg.cc_ann_tok);
            if !has && i + 1 == items.len() && !cc_args.is_empty() {
                (has, t, it) = read_placement(&cc_args);
                if has {
                    target = seg.cc_ann_tok;
                }
            }
            if kind == Some(ClassKind::Record) && !(has || is_parameter(attributes)) {
                continue;
            }
            let name = item.component.name.as_str();
            let mut n = self.make_node(name, &type_name, tref, kind, direction, &t);
            n.seg = Some(seg);
            n.target = target;
            n.has_placement = has;
            n.has_transformation = t.has;
            n.has_icon_transformation = it.has;
            n.tentative = tref.is_none();
            if t.has {
                n.has_old = true;
                (n.old_x, n.old_y) = transformation_centre(&t);
            }
            if self.opts.keep && t.has {
                n.set_transformation(&t);
                n.fixed = true;
                n.placed = true;
            }
            if !self.d.index.contains_key(name) {
                self.add_node(n, tref);
            }
        }
    }

    /// Every component inherited from a base class that has a place, as a fixed node: the parts of
    /// the class are placed clear of them and the lines routed around them.
    fn inherited_obstacles(&mut self) {
        let mut names = Vec::new();
        inherited_names(self.lib, self.class, 0, &mut names);
        for name in names {
            if !self.d.index.contains_key(&name) {
                self.inherited_node(&name, true);
            }
        }
    }

    /// The node of a component inherited from a base class. It is where the base class has it: at
    /// the place this run gives it there, or else at its placement. With `drawn`, none for a
    /// component whose class has no diagram box (a type, a function, a package).
    fn inherited_node(&mut self, name: &str, drawn: bool) -> Option<usize> {
        let d = find_component(self.lib, self.class, name, 0)?;
        if d.owner == self.class {
            return None;
        }
        let (_, mut t, _) = read_placement(&comment_args(d.item.comment.as_ref()));
        if let Some(places) = layout_of(self.lib, d.owner, self.opts)
            && let Some(b) = places.get(name)
        {
            t = Transformation {
                has: true,
                ox: b[0],
                oy: b[1],
                ex1: -b[2] / 2.0,
                ey1: -b[3] / 2.0,
                ex2: b[2] / 2.0,
                ey2: b[3] / 2.0,
                rotation: b[4],
            };
        }
        if !t.has {
            return None;
        }
        let tp = d.type_path()?;
        let tref = lookup_path(self.lib, d.owner, tp, false, 0);
        let direction = d.attributes().map_or(ACAUSAL, direction_code);
        let (kind, direction) = kind_and_causality(self.lib, tref, direction);
        if drawn && kind.is_some_and(ClassKind::is_not_drawn) {
            return None;
        }
        let mut n = self.make_node(name, &path_parts(tp).join("."), tref, kind, direction, &t);
        n.set_transformation(&t);
        n.fixed = true;
        n.placed = true;
        n.inherited = true;
        n.has_placement = true;
        n.has_transformation = true;
        Some(self.add_node(n, tref))
    }

    /// The node and port that one end of a connect equation names; no node when it is unknown.
    fn link_end(&mut self, c: &Absyn::ComponentRef) -> Option<(usize, Option<usize>)> {
        let (first, second) = connect_end(c);
        if first.is_empty() {
            return None;
        }
        let ni = match self.d.index.get(&first) {
            Some(i) => *i,
            None => self.inherited_node(&first, false)?,
        };
        let n = &self.d.nodes[ni];
        if n.is_connector || second.is_empty() {
            return Some((ni, None));
        }
        if let Some(pi) = n.port_index.get(&second) {
            return Some((ni, Some(*pi)));
        }
        let mut port = Port {
            name: second.clone(),
            ix: 0.0,
            iy: 0.0,
            known: false,
            causality: 0,
            color: String::new(),
        };
        if let Some(tref) = self.refs[ni] {
            let info = port_info(self.lib, tref, &second);
            if info.found {
                port = Port {
                    name: second,
                    ix: info.x,
                    iy: info.y,
                    known: info.known,
                    causality: info.causality,
                    color: info.color,
                };
            }
        }
        Some((ni, Some(self.d.nodes[ni].add_port(port))))
    }

    fn extract_connect(
        &mut self,
        comment: Option<&Ref<Absyn::Comment>>,
        info: &Absyn::Info,
        c1: &Absyn::ComponentRef,
        c2: &Absyn::ComponentRef,
    ) {
        let ea = self.link_end(c1);
        let eb = self.link_end(c2);
        let (Some((a, pa)), Some((b, pb))) = (ea, eb) else {
            self.warnings.push(format!(
                "{}: connect({}, {}): component not found; no line",
                self.d.class_name,
                connect_end(c1).0,
                connect_end(c2).0
            ));
            return;
        };
        if a == b && pa == pb {
            return;
        }
        let src = self.src;
        let mut eq_tok = start_token(src, info).filter(|t| matches!(src.kind(*t), TokenKind::Connect));
        let (semi, ann) = match eq_tok {
            Some(t) => src.equation_end(t),
            None => (None, None),
        };
        if semi.is_none() {
            eq_tok = None;
        }
        let has_line = find_modifier(&comment_args(comment), "Line").is_some();
        let na = &self.d.nodes[a];
        let nb = &self.d.nodes[b];
        let mut color = match pa {
            None => na.color.clone(),
            Some(p) => na.ports[p].color.clone(),
        };
        if color.is_empty() {
            color = match pb {
                None => nb.color.clone(),
                Some(p) => nb.ports[p].color.clone(),
            };
        }
        let keep = self.opts.keep && has_line && na.fixed && nb.fixed;
        self.d.links.push(Link {
            eq_tok,
            semi_tok: semi,
            ann_tok: ann,
            color,
            keep,
            ..Link::new(a, pa, b, pb)
        });
    }

    fn extract_equations(&mut self, items: &List<Ref<Absyn::EquationItem>>) {
        for it in items {
            let Absyn::EQUATIONITEM {
                equation_,
                comment,
                info,
            } = &**it
            else {
                continue;
            };
            match &**equation_ {
                Absyn::EQ_CONNECT { connector1, connector2 } => {
                    self.extract_connect(comment.as_ref(), info, connector1, connector2)
                }
                Absyn::EQ_FOR {
                    forEquations: for_equations,
                    ..
                } => self.extract_equations(for_equations),
                Absyn::EQ_IF {
                    equationTrueItems: true_items,
                    elseIfBranches: else_if_branches,
                    equationElseItems: else_items,
                    ..
                } => {
                    self.extract_equations(true_items);
                    for br in else_if_branches {
                        self.extract_equations(&br.1);
                    }
                    self.extract_equations(else_items);
                }
                _ => {}
            }
        }
    }

    /// Remove the nodes whose type did not resolve and that nothing refers to.
    fn drop_tentative(&mut self) {
        let count = self.d.nodes.len();
        let mut used = vec![false; count];
        for l in &self.d.links {
            used[l.a] = true;
            used[l.b] = true;
        }
        let mut map = vec![0usize; count];
        let mut nodes: Vec<Node> = Vec::new();
        let mut refs: Vec<Option<ClassId>> = Vec::new();
        for (i, n) in std::mem::take(&mut self.d.nodes).into_iter().enumerate() {
            if n.tentative && !used[i] && !n.has_placement {
                continue;
            }
            if n.tentative {
                self.warnings.push(format!(
                    "{}: class {} of {} not found; default size, ports unknown",
                    self.d.class_name, n.type_name, n.name
                ));
            }
            map[i] = nodes.len();
            nodes.push(n);
            refs.push(self.refs[i]);
        }
        let dropped = nodes.len() != count;
        self.d.nodes = nodes;
        if !dropped {
            return;
        }
        self.refs = refs;
        self.d.index.clear();
        for (i, n) in self.d.nodes.iter().enumerate() {
            self.d.index.insert(n.name.clone(), i);
        }
        for l in &mut self.d.links {
            l.a = map[l.a];
            l.b = map[l.b];
        }
    }
}

/// The source of `file`: the text being laid out, or the file as it is on disk.
pub fn source_of(lib: &mut Library, file: &str) -> Option<Rc<Source>> {
    let key = file_path(file);
    if let Some(s) = lib.sources.get(&key) {
        return s.clone();
    }
    let src = if file.is_empty() {
        None
    } else {
        read_text(file)
            .and_then(|text| Source::new(&text, file).ok())
            .map(Rc::new)
    };
    lib.sources.insert(key, src.clone());
    src
}

/// Where the own components of class `c` are after this run: name to (x, y, width, height,
/// rotation). None when the run does not lay out `c`: then its components are where their
/// placements say.
pub fn layout_of(lib: &mut Library, c: ClassId, opts: &Options) -> Option<Rc<LayoutPlaces>> {
    let name = lib.classes[c].name.clone();
    if let Some(l) = lib.layout_cache.get(&name) {
        return l.clone();
    }
    lib.layout_cache.insert(name.clone(), None);
    let file = lib.classes[c].file.clone();
    if !(lib.targets.contains(&file_path(&file)) && class_selected(&name, &opts.classes)) {
        return None;
    }
    if !restriction_kind(&lib.classes[c].class).has_diagram() {
        return None;
    }
    let src = source_of(lib, &file)?;
    let mut d = extract_diagram(lib, &src, c, opts, &mut Vec::new())?;
    place_diagram(&mut d, opts);
    let mut out = LayoutPlaces::new();
    for n in &d.nodes {
        if !n.inherited {
            out.insert(
                n.name.clone(),
                [n.x, n.y, n.extent_width(), n.extent_height(), f64::from(n.rot)],
            );
        }
    }
    let out = Rc::new(out);
    lib.layout_cache.insert(name, Some(out.clone()));
    Some(out)
}

/// True for a class that has only connectors, all with a place, and connects nothing.
pub fn is_interface(d: &Diagram) -> bool {
    d.links.is_empty()
        && d.nodes
            .iter()
            .filter(|n| !n.inherited)
            .all(|n| n.is_connector && n.has_transformation)
}

/// The names of the components that class `c` inherits, from its base classes and theirs, in
/// order and without repeats; components of a built-in type are left out.
fn inherited_names(lib: &mut Library, c: ClassId, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_LOOKUP_DEPTH {
        return;
    }
    for b in base_classes(lib, c, depth) {
        let elements = lib.elements_of(b);
        for e in elements.iter() {
            let Absyn::ELEMENT { specification, .. } = &**e else {
                continue;
            };
            let Absyn::COMPONENTS {
                typeSpec: type_spec,
                components,
                ..
            } = &**specification
            else {
                continue;
            };
            if is_builtin_path(&path_parts(type_path(type_spec))) {
                continue;
            }
            for item in components {
                let name = item.component.name.as_str();
                if !out.iter().any(|o| o == name) {
                    out.push(name.to_owned());
                }
            }
        }
        inherited_names(lib, b, depth + 1, out);
    }
}

/// The diagram of class `c`, or none when it has nothing to lay out: no components, or only
/// connectors that have their places (an interface, which the classes that extend it rely on).
pub fn extract_diagram(
    lib: &mut Library,
    src: &Source,
    c: ClassId,
    opts: &Options,
    warnings: &mut Vec<String>,
) -> Option<Diagram> {
    let class = lib.classes[c].class.clone();
    let Absyn::PARTS { classParts: parts, .. } = &*class.body else {
        return None;
    };
    let mut d = Diagram::new(&lib.classes[c].name);
    d.icon_places = (*icon_places(lib, c)).clone();
    let mut x = Extraction {
        lib,
        src,
        class: c,
        d,
        refs: Vec::new(),
        opts,
        warnings,
    };
    for part in parts {
        if let Absyn::PUBLIC { contents } | Absyn::PROTECTED { contents } = &**part {
            for item in contents {
                if let Absyn::ELEMENTITEM { element } = &**item {
                    x.extract_element(element);
                }
            }
        }
    }
    for part in parts {
        if let Absyn::EQUATIONS { contents } = &**part {
            x.extract_equations(contents);
        }
    }
    x.drop_tentative();
    x.inherited_obstacles();
    let mut d = x.d;
    if (d.nodes.iter().all(|n| n.inherited) && d.links.is_empty()) || is_interface(&d) {
        return None;
    }
    d.end_tok = class_end_token(src, &class);
    d.ann_lpar = class_annotation_lpar(src, &class);
    if let Some(cs) = find_modifier_path(&class_annotation_args(&class), &["Diagram", "coordinateSystem"]) {
        let ext = modifier_numbers(&modifier_args(&cs), "extent");
        if ext.len() == 4 {
            d.has_extent = true;
            d.x1 = ext[0].min(ext[2]);
            d.y1 = ext[1].min(ext[3]);
            d.x2 = ext[0].max(ext[2]);
            d.y2 = ext[1].max(ext[3]);
        }
    }
    Some(d)
}

/// The classes of `c` and below that can have a diagram, in source order.
pub fn diagram_classes(out: &mut Vec<ClassId>, lib: &mut Library, c: ClassId, depth: usize) {
    if depth > MAX_LOOKUP_DEPTH {
        return;
    }
    let class = lib.classes[c].class.clone();
    if !matches!(&*class.body, Absyn::PARTS { .. }) {
        return;
    }
    if restriction_kind(&class).has_diagram() {
        out.push(c);
    }
    let elements = lib.elements_of(c);
    for e in elements.iter() {
        if let Absyn::ELEMENT { specification, .. } = &**e
            && let Absyn::CLASSDEF { class_: cls, .. } = &**specification
            && let Some(sub) = lib.local_class(c, &cls.name)
            && Ref::ptr_eq(&lib.classes[sub].class, cls)
        {
            diagram_classes(out, lib, sub, depth + 1);
        }
    }
}

/// True when the class name is selected by `names` (all when empty): the name, a suffix, or an
/// enclosing package.
pub fn class_selected(name: &str, names: &[String]) -> bool {
    names.is_empty()
        || names.iter().any(|s| {
            name == s
                || name.strip_suffix(s.as_str()).is_some_and(|r| r.ends_with('.'))
                || name.strip_prefix(s.as_str()).is_some_and(|r| r.starts_with('.'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::lookup::parse_text;
    use crate::source::resolve::register_program;

    const TEXT: &str = "package P
  connector In = input Real;
  connector Out = output Real;
  block Gain
    In u annotation(Placement(transformation(origin={-120,0}, extent={{-20,-20},{20,20}})));
    Out y annotation(Placement(transformation(origin={110,0}, extent={{-10,-10},{10,10}})));
    parameter Real k = 1;
  end Gain;
  model Chain
    Gain a(k = 2), b;
    Nope.Gain n;
    Nope.Gain used;
  equation
    connect(a.y, b.u);
    connect(b.y, used.u) annotation(Line(points={{0,0},{1,1}}));
    connect(a.y, missing.u);
  end Chain;
end P;";

    #[test]
    fn selection() {
        let names = vec!["P.A".to_owned()];
        assert!(class_selected("P.A", &names));
        assert!(class_selected("Q.P.A", &names));
        assert!(class_selected("P.A.B", &names));
        assert!(!class_selected("P.AB", &names));
        assert!(!class_selected("XP.A", &names));
        assert!(class_selected("anything", &[]));
    }

    #[test]
    fn diagram_of_a_class() {
        let mut lib = Library::new(&[]);
        let prog = match parse_text(TEXT, "") {
            Ok(p) => p,
            Err(e) => panic!("{e}"),
        };
        let src = match Source::new(TEXT, "") {
            Ok(s) => s,
            Err(e) => panic!("{e}"),
        };
        let ids = register_program(&mut lib, &prog, "");
        let mut classes = Vec::new();
        diagram_classes(&mut classes, &mut lib, ids[0], 0);
        let names: Vec<&str> = classes.iter().map(|c| lib.classes[*c].name.as_str()).collect();
        assert_eq!(names, ["P.Gain", "P.Chain"]);
        let opts = Options::default();
        let mut warnings = Vec::new();
        // Gain has only placed connectors: an interface.
        assert!(extract_diagram(&mut lib, &src, classes[0], &opts, &mut warnings).is_none());
        let d = extract_diagram(&mut lib, &src, classes[1], &opts, &mut warnings);
        let Some(d) = d else {
            panic!("Chain has a diagram");
        };
        let nodes: Vec<&str> = d.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(nodes, ["a", "b", "used"]);
        assert_eq!(d.links.len(), 2);
        assert_eq!((d.links[0].a, d.links[0].b), (0, 1));
        assert!(d.links[0].eq_tok.is_some() && d.links[0].ann_tok.is_none());
        assert!(d.links[1].ann_tok.is_some());
        let y = &d.nodes[0].ports[0];
        assert_eq!(
            (y.name.as_str(), y.ix, y.iy, y.known, y.causality),
            ("y", 110.0, 0.0, true, 2)
        );
        assert_eq!(
            warnings,
            [
                "P.Chain: connect(a, missing): component not found; no line",
                "P.Chain: class Nope.Gain of used not found; default size, ports unknown"
            ]
        );
        let end = d.end_tok.map(|t| (src.token_text(t), src.token_text(t + 1)));
        assert_eq!(end, Some(("end", "Chain")));
        assert_eq!(d.ann_lpar, None);
    }
}
