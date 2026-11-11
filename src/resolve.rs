// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Name lookup, and the facts the layout reads from a resolved class.
//! Mirrors `src/resolve.jl` of ModelicaComponentAutolayout.jl.

use std::path::Path;
use std::rc::Rc;

use metamodelica::Ref;
use openmodelica_ast::Absyn;

use crate::absyn::{
    Args, NO_TRANSFORMATION, Transformation, class_annotation_args, comment_args, find_modifier, find_modifier_path,
    icon_line_color, modifier_args, modifier_bool, modifier_numbers, path_parts, read_transformation, rotate_point,
    type_path,
};
use crate::diagram::{ACAUSAL, INPUT, OUTPUT};
use crate::lookup::{
    ClassId, DEFAULT_ICON_SYSTEM, IconPlaces, IconSystem, Library, PortInfo, abs_path, dir_name, is_dir, is_file,
};

pub const MAX_LOOKUP_DEPTH: usize = 24;

pub fn is_builtin_type(name: &str) -> bool {
    matches!(name, "Real" | "Integer" | "Boolean" | "String" | "Clock")
}

/// Whether the type path `parts` names a builtin type (or nothing).
pub fn is_builtin_path(parts: &[String]) -> bool {
    match parts {
        [] => true,
        [name] => is_builtin_type(name),
        _ => false,
    }
}

/// The class at the fully qualified name `parts`.
pub fn lookup_absolute(lib: &mut Library, parts: &[String]) -> Option<ClassId> {
    let (first, rest) = parts.split_first()?;
    let mut cur = lib.top_level(first)?;
    for p in rest {
        cur = lookup_member(lib, cur, p, 1)?;
    }
    Some(cur)
}

/// The class that the imports of `c` give the name.
pub fn lookup_imports(lib: &mut Library, c: ClassId, name: &str) -> Option<ClassId> {
    let mut unqualified: Vec<Vec<String>> = Vec::new();
    let elements = lib.elements_of(c);
    for e in elements.iter() {
        let Absyn::ELEMENT { specification, .. } = &**e else {
            continue;
        };
        let Absyn::IMPORT { import_: imp, .. } = &**specification else {
            continue;
        };
        match imp {
            Absyn::NAMED_IMPORT { name: n, path } => {
                if n == name {
                    return lookup_absolute(lib, &path_parts(path));
                }
            }
            Absyn::QUAL_IMPORT { path } => {
                let parts = path_parts(path);
                if parts.last().is_some_and(|l| l == name) {
                    return lookup_absolute(lib, &parts);
                }
            }
            Absyn::UNQUAL_IMPORT { path } => unqualified.push(path_parts(path)),
            Absyn::GROUP_IMPORT { prefix, groups } => {
                for g in groups {
                    let member = match g {
                        Absyn::GROUP_IMPORT_NAME { name: n } if n == name => Some(n),
                        Absyn::GROUP_IMPORT_RENAME { rename, name: n } if rename == name => Some(n),
                        _ => None,
                    };
                    if let Some(member) = member.filter(|m| !m.is_empty()) {
                        let mut gp = path_parts(prefix);
                        gp.push(member.to_string());
                        return lookup_absolute(lib, &gp);
                    }
                }
            }
        }
    }
    for up in &unqualified {
        let Some(pkg) = lookup_absolute(lib, up) else {
            continue;
        };
        if let Some(r) = lookup_member(lib, pkg, name, 1) {
            return Some(r);
        }
    }
    None
}

/// The base classes of `c`: its extends clauses, and the class a short class definition names.
pub fn base_classes(lib: &mut Library, c: ClassId, depth: usize) -> Vec<ClassId> {
    let mut out = Vec::new();
    if depth > MAX_LOOKUP_DEPTH {
        return out;
    }
    let class = lib.classes[c].class.clone();
    if let Absyn::DERIVED {
        typeSpec: type_spec, ..
    } = &*class.body
    {
        if let Some(t) = lookup_path(lib, c, type_path(type_spec), true, depth + 1)
            && t != c
        {
            out.push(t);
        }
        return out;
    }
    let elements = lib.elements_of(c);
    for e in elements.iter() {
        if let Absyn::ELEMENT { specification, .. } = &**e
            && let Absyn::EXTENDS { path, .. } = &**specification
            && let Some(b) = lookup_path(lib, c, path, true, depth + 1)
            && b != c
        {
            out.push(b);
        }
    }
    out
}

/// The class `name` that is a member of `c`: declared in it or inherited.
pub fn lookup_member(lib: &mut Library, c: ClassId, name: &str, depth: usize) -> Option<ClassId> {
    if depth > MAX_LOOKUP_DEPTH {
        return None;
    }
    if let Some(r) = lib.local_class(c, name) {
        return Some(r);
    }
    for b in base_classes(lib, c, depth) {
        if let Some(r) = lookup_member(lib, b, name, depth + 1) {
            return Some(r);
        }
    }
    None
}

/// The class that `name` means inside `scope`. `skip_inherited` leaves out the members `scope` inherits.
pub fn lookup_first(
    lib: &mut Library,
    scope: ClassId,
    name: &str,
    skip_inherited: bool,
    depth: usize,
) -> Option<ClassId> {
    if depth > MAX_LOOKUP_DEPTH {
        return None;
    }
    let mut s = Some(scope);
    let mut first = true;
    while let Some(cur) = s {
        if let Some(r) = lib.local_class(cur, name) {
            return Some(r);
        }
        if let Some(r) = lookup_imports(lib, cur, name) {
            return Some(r);
        }
        if !(first && skip_inherited) {
            for b in base_classes(lib, cur, depth) {
                if let Some(r) = lookup_member(lib, b, name, depth + 1) {
                    return Some(r);
                }
            }
        }
        first = false;
        s = lib.classes[cur].parent;
    }
    lib.top_level(name)
}

/// The class that `path` means inside `scope`.
pub fn lookup_path(
    lib: &mut Library,
    scope: ClassId,
    path: &Absyn::Path,
    skip_inherited: bool,
    depth: usize,
) -> Option<ClassId> {
    let parts = path_parts(path);
    let first = parts.first()?;
    let fq = matches!(path, Absyn::FULLYQUALIFIED { .. });
    let key = if fq {
        format!(".{}", parts.join("."))
    } else {
        let mark = if skip_inherited { "!" } else { "|" };
        format!("{}{}{}", lib.classes[scope].name, mark, parts.join("."))
    };
    if let Some(r) = lib.path_cache.get(&key) {
        return *r;
    }
    if depth > MAX_LOOKUP_DEPTH {
        return None;
    }
    let mut cur = if fq {
        lib.top_level(first)
    } else {
        lookup_first(lib, scope, first, skip_inherited, depth)
    };
    for p in &parts[1..] {
        let Some(c) = cur else {
            break;
        };
        cur = lookup_member(lib, c, p, depth + 1);
    }
    if cur.is_some() || depth == 0 {
        lib.path_cache.insert(key, cur);
    }
    cur
}

/// The restriction of a class, as far as the layout cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassKind {
    Model,
    Block,
    Class,
    Connector,
    ExpandableConnector,
    Record,
    Type,
    Package,
    Function,
    Enumeration,
    Other,
}

impl ClassKind {
    /// A connector or an expandable connector.
    pub fn is_connector(self) -> bool {
        matches!(self, ClassKind::Connector | ClassKind::ExpandableConnector)
    }

    /// A model, block or class: a class whose diagram is laid out.
    pub fn has_diagram(self) -> bool {
        matches!(self, ClassKind::Model | ClassKind::Block | ClassKind::Class)
    }

    /// A type, package, function, enumeration or other class: its components are not in a diagram.
    pub fn is_not_drawn(self) -> bool {
        matches!(
            self,
            ClassKind::Type | ClassKind::Package | ClassKind::Function | ClassKind::Enumeration | ClassKind::Other
        )
    }
}

/// The kind of a class by its restriction.
pub fn restriction_kind(c: &Absyn::Class) -> ClassKind {
    use Absyn::Restriction as R;
    if matches!(&*c.body, Absyn::ENUMERATION { .. }) {
        return ClassKind::Enumeration;
    }
    match &c.restriction {
        R::R_MODEL => ClassKind::Model,
        R::R_BLOCK => ClassKind::Block,
        R::R_CLASS => ClassKind::Class,
        R::R_CONNECTOR => ClassKind::Connector,
        R::R_EXP_CONNECTOR => ClassKind::ExpandableConnector,
        R::R_RECORD | R::R_OPERATOR_RECORD => ClassKind::Record,
        R::R_TYPE => ClassKind::Type,
        R::R_PACKAGE => ClassKind::Package,
        R::R_FUNCTION { .. } => ClassKind::Function,
        R::R_ENUMERATION => ClassKind::Enumeration,
        _ => ClassKind::Other,
    }
}

/// The causality given by a direction prefix: `INPUT`, `OUTPUT` or `ACAUSAL`.
pub fn direction_code(a: &Absyn::ElementAttributes) -> i32 {
    match a.direction {
        Absyn::Direction::INPUT => INPUT,
        Absyn::Direction::OUTPUT => OUTPUT,
        _ => ACAUSAL,
    }
}

/// The kind of a class and the direction prefix it carries, read along its short class definitions.
/// A `class` that is short for another class has that one's kind.
pub fn class_kind(lib: &mut Library, c: ClassId) -> (ClassKind, i32) {
    let mut cur = c;
    let mut kind = None;
    let mut direction = ACAUSAL;
    for _ in 0..MAX_LOOKUP_DEPTH {
        let class = lib.classes[cur].class.clone();
        let k = restriction_kind(&class);
        if kind.is_none() && k != ClassKind::Class {
            kind = Some(k);
        }
        let Absyn::DERIVED { attributes, .. } = &*class.body else {
            break;
        };
        if direction == ACAUSAL {
            direction = direction_code(attributes);
        }
        let bases = base_classes(lib, cur, 1);
        let Some(b) = bases.first() else {
            break;
        };
        cur = *b;
    }
    (kind.unwrap_or(ClassKind::Class), direction)
}

/// The kind of the class `tref` (none when it is unknown) and the causality of a component of it:
/// the component's own direction `direction`, else the one its class carries.
pub fn kind_and_causality(lib: &mut Library, tref: Option<ClassId>, direction: i32) -> (Option<ClassKind>, i32) {
    let Some(c) = tref else {
        return (None, direction);
    };
    let (kind, class_direction) = class_kind(lib, c);
    (
        Some(kind),
        if direction == ACAUSAL {
            class_direction
        } else {
            direction
        },
    )
}

/// A component declaration found in `owner`.
#[derive(Clone)]
pub struct CompDecl {
    pub owner: ClassId,
    /// A `COMPONENTS` specification.
    pub spec: Ref<Absyn::ElementSpec>,
    pub item: Ref<Absyn::ComponentItem>,
}

impl CompDecl {
    pub fn attributes(&self) -> Option<&Absyn::ElementAttributes> {
        match &*self.spec {
            Absyn::COMPONENTS { attributes, .. } => Some(attributes),
            _ => None,
        }
    }

    pub fn type_path(&self) -> Option<&Absyn::Path> {
        match &*self.spec {
            Absyn::COMPONENTS {
                typeSpec: type_spec, ..
            } => Some(type_path(type_spec)),
            _ => None,
        }
    }
}

/// The declaration of the component `name` of `c`, its own or inherited.
pub fn find_component(lib: &mut Library, c: ClassId, name: &str, depth: usize) -> Option<CompDecl> {
    if depth > MAX_LOOKUP_DEPTH {
        return None;
    }
    let elements = lib.elements_of(c);
    for e in elements.iter() {
        let Absyn::ELEMENT { specification, .. } = &**e else {
            continue;
        };
        let Absyn::COMPONENTS { components, .. } = &**specification else {
            continue;
        };
        for item in components {
            if item.component.name == name {
                return Some(CompDecl {
                    owner: c,
                    spec: specification.clone(),
                    item: item.clone(),
                });
            }
        }
    }
    for b in base_classes(lib, c, depth) {
        if let Some(r) = find_component(lib, b, name, depth + 1) {
            return Some(r);
        }
    }
    None
}

/// The Placement of a declaration: (found, transformation, iconTransformation).
pub fn read_placement(args: &[Ref<Absyn::ElementArg>]) -> (bool, Transformation, Transformation) {
    let Some(pm) = find_modifier(args, "Placement") else {
        return (false, NO_TRANSFORMATION, NO_TRANSFORMATION);
    };
    let pargs: Args = modifier_args(&pm);
    let t = match find_modifier(&pargs, "transformation") {
        Some(tm) => read_transformation(&modifier_args(&tm)),
        None => NO_TRANSFORMATION,
    };
    let it = match find_modifier(&pargs, "iconTransformation") {
        Some(im) => read_transformation(&modifier_args(&im)),
        None => NO_TRANSFORMATION,
    };
    (true, t, it)
}

/// The centre of a transformation's extent in the coordinates around it.
pub fn transformation_centre(t: &Transformation) -> (f64, f64) {
    let r = rotate_point((t.ex1 + t.ex2) / 2.0, (t.ey1 + t.ey2) / 2.0, t.rotation);
    (t.ox + r.0, t.oy + r.1)
}

/// The line colour of a class's icon: its own, or the first one a base class has.
pub fn class_color(lib: &mut Library, c: ClassId, depth: usize) -> String {
    if depth > MAX_LOOKUP_DEPTH {
        return String::new();
    }
    let class = lib.classes[c].class.clone();
    let s = icon_line_color(&class_annotation_args(&class));
    if !s.is_empty() {
        return s;
    }
    for b in base_classes(lib, c, depth) {
        let s = class_color(lib, b, depth + 1);
        if !s.is_empty() {
            return s;
        }
    }
    String::new()
}

/// The direction and line colour of a component declaration's connector class.
pub fn connector_traits(lib: &mut Library, d: &CompDecl) -> (i32, String) {
    let mut causality = d.attributes().map_or(ACAUSAL, direction_code);
    let mut color = String::new();
    if let Some(tp) = d.type_path() {
        let parts = path_parts(tp);
        if !is_builtin_path(&parts)
            && let Some(tref) = lookup_path(lib, d.owner, tp, false, 0)
        {
            (_, causality) = kind_and_causality(lib, Some(tref), causality);
            color = class_color(lib, tref, 0);
        }
    }
    (causality, color)
}

/// The places in the icon of `c` for its own connectors without a place (no `transformation` and
/// no `iconTransformation` with an extent), by name: (x, y, width, height). Inputs are on the left
/// border, outputs on the right, the others half on each side. The layout writes these places as
/// `iconTransformation`, and uses them for the ports of a component of class `c` until then.
pub fn icon_places(lib: &mut Library, c: ClassId) -> Rc<IconPlaces> {
    let key = lib.classes[c].name.clone();
    if let Some(p) = lib.place_cache.get(&key) {
        return p.clone();
    }
    let mut names: Vec<String> = Vec::new();
    let mut causality: Vec<i32> = Vec::new();
    let mut sizes: Vec<(f64, f64)> = Vec::new();
    let elements = lib.elements_of(c);
    for e in elements.iter() {
        let Absyn::ELEMENT {
            specification,
            innerOuter: inner_outer,
            ..
        } = &**e
        else {
            continue;
        };
        if matches!(inner_outer, Absyn::InnerOuter::OUTER) {
            continue;
        }
        let Absyn::COMPONENTS {
            attributes,
            typeSpec: type_spec,
            components,
        } = &**specification
        else {
            continue;
        };
        let tp = type_path(type_spec);
        let parts = path_parts(tp);
        if is_builtin_path(&parts) {
            continue;
        }
        let Some(tref) = lookup_path(lib, c, tp, false, 0) else {
            continue;
        };
        let (kind, direction) = kind_and_causality(lib, Some(tref), direction_code(attributes));
        if !kind.is_some_and(ClassKind::is_connector) {
            continue;
        }
        let (w, h) = icon_system(lib, tref).default_size(1.0);
        for item in components {
            let (_, t, it) = read_placement(&comment_args(item.comment.as_ref()));
            if t.has || it.has {
                continue;
            }
            let name = item.component.name.as_str();
            if names.iter().any(|n| n == name) {
                continue;
            }
            names.push(name.to_owned());
            causality.push(direction);
            sizes.push((w, h));
        }
    }
    let mut left: Vec<usize> = Vec::new();
    let mut right: Vec<usize> = Vec::new();
    let mut other: Vec<usize> = Vec::new();
    for (i, cz) in causality.iter().enumerate() {
        match *cz {
            INPUT => left.push(i),
            OUTPUT => right.push(i),
            _ => other.push(i),
        }
    }
    let half = other.len().div_ceil(2);
    for (k, i) in other.iter().enumerate() {
        if k < half {
            left.push(*i);
        } else {
            right.push(*i);
        }
    }
    let mut out = IconPlaces::new();
    for (side, ids) in [(INPUT, &left), (OUTPUT, &right)] {
        let m = ids.len();
        for (k0, &i) in ids.iter().enumerate() {
            let k = (k0 + 1) as f64;
            let mut x = if side == INPUT { -100.0 } else { 100.0 };
            if causality[i] == INPUT {
                x = -100.0 - sizes[i].0 / 2.0;
            }
            if causality[i] == OUTPUT {
                x = 100.0 + sizes[i].0 / 2.0;
            }
            let y = ((100.0 - 200.0 * k / (m + 1) as f64) / 10.0).round_ties_even() * 10.0;
            out.insert(names[i].clone(), (x, y, sizes[i].0, sizes[i].1));
        }
    }
    let out = Rc::new(out);
    lib.place_cache.insert(key, out.clone());
    out
}

/// Where the connector `name` of class `c` sits in the icon of `c`.
pub fn port_info(lib: &mut Library, c: ClassId, name: &str) -> PortInfo {
    let key = format!("{}#{}", lib.classes[c].name, name);
    if let Some(p) = lib.port_cache.get(&key) {
        return p.clone();
    }
    let mut info = PortInfo {
        found: false,
        known: false,
        x: 0.0,
        y: 0.0,
        causality: 0,
        color: String::new(),
    };
    if let Some(d) = find_component(lib, c, name, 0) {
        let (_, t, it) = read_placement(&comment_args(d.item.comment.as_ref()));
        let used = if it.has { it } else { t };
        let mut pos = if used.has {
            transformation_centre(&used)
        } else {
            (0.0, 0.0)
        };
        let mut known = used.has;
        if !used.has {
            let places = icon_places(lib, d.owner);
            if let Some(p) = places.get(name) {
                pos = (p.0, p.1);
                known = true;
            }
        }
        let (causality, color) = connector_traits(lib, &d);
        info = PortInfo {
            found: true,
            known,
            x: pos.0,
            y: pos.1,
            causality,
            color,
        };
    }
    lib.port_cache.insert(key, info.clone());
    info
}

fn icon_system_of(lib: &mut Library, c: ClassId, depth: usize) -> IconSystem {
    if depth > MAX_LOOKUP_DEPTH {
        return DEFAULT_ICON_SYSTEM;
    }
    let class = lib.classes[c].class.clone();
    if let Some(cs) = find_modifier_path(&class_annotation_args(&class), &["Icon", "coordinateSystem"]) {
        let cargs = modifier_args(&cs);
        let ext = modifier_numbers(&cargs, "extent");
        let sc = modifier_numbers(&cargs, "initialScale");
        let pres = modifier_bool(&cargs, "preserveAspectRatio", true);
        let scale = if sc.len() == 1 && sc[0] > 0.0 { sc[0] } else { 0.1 };
        let (x1, y1, x2, y2) = if ext.len() == 4 && ext[0] != ext[2] && ext[1] != ext[3] {
            (ext[0], ext[1], ext[2], ext[3])
        } else {
            (-100.0, -100.0, 100.0, 100.0)
        };
        return IconSystem {
            found: true,
            x1,
            y1,
            x2,
            y2,
            preserve_aspect_ratio: pres,
            initial_scale: scale,
        };
    }
    for b in base_classes(lib, c, depth) {
        let s = icon_system_of(lib, b, depth + 1);
        if s.found {
            return s;
        }
    }
    DEFAULT_ICON_SYSTEM
}

/// The coordinate system of the icon of `c`: its own, or the first one a base class gives.
pub fn icon_system(lib: &mut Library, c: ClassId) -> IconSystem {
    if let Some(s) = lib.icon_cache.get(&lib.classes[c].name) {
        return *s;
    }
    let s = icon_system_of(lib, c, 0);
    lib.icon_cache.insert(lib.classes[c].name.clone(), s);
    s
}

/// Enter the classes of a parsed file into the library, below its `within` class. Returns them in
/// file order. The directory that holds the file's top-level package becomes a library root.
pub fn register_program(lib: &mut Library, prog: &Absyn::Program, file: &str) -> Vec<ClassId> {
    let mut out = Vec::new();
    let on_disk = !file.is_empty() && is_file(file);
    let dir0 = if on_disk {
        dir_name(&abs_path(file))
    } else {
        String::new()
    };
    let is_pkg = on_disk && Path::new(file).file_name().is_some_and(|n| n == "package.mo");
    let prefix: Vec<String> = match &prog.within_ {
        Absyn::WITHIN { path } => path_parts(path),
        _ => Vec::new(),
    };
    if on_disk {
        let mut root = if is_pkg { dir_name(&dir0) } else { dir0.clone() };
        for _ in 0..prefix.len() {
            root = dir_name(&root);
        }
        if is_dir(&root) && !lib.roots.contains(&root) {
            lib.roots.insert(0, root);
        }
    }
    let parent = if prefix.is_empty() {
        None
    } else {
        lookup_absolute(lib, &prefix)
    };
    for cls in &prog.classes {
        let name = if prefix.is_empty() {
            cls.name.to_string()
        } else {
            format!("{}.{}", prefix.join("."), cls.name)
        };
        let id = lib.add_class(
            cls.clone(),
            name,
            if is_pkg { dir0.clone() } else { String::new() },
            parent,
            if on_disk { file.to_owned() } else { String::new() },
        );
        if let Some(p) = parent {
            lib.local_class(p, &cls.name);
            lib.classes[p].locals.insert(cls.name.to_string(), Some(id));
        } else if prefix.is_empty() {
            lib.top.insert(cls.name.to_string(), Some(id));
        }
        lib.note_uses(cls);
        out.push(id);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookup::parse_text;

    fn library(text: &str) -> (Library, Vec<ClassId>) {
        let mut lib = Library::new(&[]);
        let ids = match parse_text(text, "") {
            Ok(prog) => register_program(&mut lib, &prog, ""),
            Err(e) => panic!("{e}"),
        };
        (lib, ids)
    }

    const TEXT: &str = "package P
  connector In = input Real annotation(Icon(coordinateSystem(extent={{-50,-50},{50,50}})));
  connector Out = output Real;
  partial block Base
    In u annotation(Placement(transformation(origin={-120,0}, extent={{-20,-20},{20,20}})));
  end Base;
  block Gain
    extends Base;
    Out y;
    Out z;
  end Gain;
  model M
    import P.Gain;
    Gain g;
  end M;
end P;";

    #[test]
    fn lookup_and_ports() {
        let (mut lib, ids) = library(TEXT);
        let p = ids[0];
        let gain = lookup_absolute(&mut lib, &["P".to_owned(), "Gain".to_owned()]);
        assert!(gain.is_some());
        let Some(gain) = gain else { return };
        assert_eq!(class_kind(&mut lib, gain), (ClassKind::Block, ACAUSAL));
        let input = lib.local_class(p, "In");
        assert_eq!(
            input.map(|c| class_kind(&mut lib, c)),
            Some((ClassKind::Connector, INPUT))
        );
        let u = port_info(&mut lib, gain, "u");
        assert!(u.found && u.known);
        assert_eq!((u.x, u.y, u.causality), (-120.0, 0.0, 1));
        let y = port_info(&mut lib, gain, "y");
        assert!(y.found && y.known);
        assert_eq!((y.x, y.y, y.causality), (110.0, 30.0, 2));
        let z = port_info(&mut lib, gain, "z");
        assert_eq!((z.x, z.y), (110.0, -30.0));
        assert!(!port_info(&mut lib, gain, "nope").found);
        assert_eq!(input.map(|c| icon_system(&mut lib, c).x1), Some(-50.0));
        let m = lib.local_class(p, "M");
        let found = m.and_then(|m| lookup_first(&mut lib, m, "Gain", false, 0));
        assert_eq!(found, Some(gain));
    }
}
