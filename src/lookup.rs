// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Class lookup: the classes of the input, and library classes read from disk on demand.
//!
//! The layout needs three facts about the class of a component: what kind it is, where its
//! connectors sit in its icon, and the icon's coordinate system. These are found by name lookup
//! through enclosing classes, imports and base classes. Modifiers and redeclarations are not
//! applied: a name the lookup does not find is reported as unknown, and the layout does without.
//!
//! Mirrors `src/lookup.jl` of ModelicaComponentAutolayout.jl.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use metamodelica::Ref;
use openmodelica_ast::Absyn;
use openmodelica_ast::parser;

use crate::absyn::{class_annotation_args, find_modifier, modifier_args, modifier_exp};
use crate::tokens::Source;

/// A class of the library: an index into [`Library::classes`].
pub type ClassId = usize;

/// A class with its place in the class tree and on disk.
pub struct ClassRef {
    pub class: Ref<Absyn::Class>,
    /// Fully qualified name.
    pub name: String,
    /// The directory of a class stored as `dir/package.mo`, else "".
    pub dir: String,
    pub parent: Option<ClassId>,
    pub file: String,
    elements: Option<Rc<Vec<Ref<Absyn::Element>>>>,
    pub(crate) locals: HashMap<String, Option<ClassId>>,
    locals_loaded: bool,
}

/// What the layout needs of a connector of a class.
#[derive(Clone, Debug)]
pub struct PortInfo {
    pub found: bool,
    /// False when the connector has no placement.
    pub known: bool,
    pub x: f64,
    pub y: f64,
    pub causality: i32,
    pub color: String,
}

/// The coordinate system of a class's icon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconSystem {
    pub found: bool,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub preserve_aspect_ratio: bool,
    pub initial_scale: f64,
}

impl IconSystem {
    /// The default size of a component of a class with this icon system: (width, height).
    pub fn default_size(&self, scale: f64) -> (f64, f64) {
        let w = (self.x2 - self.x1).abs() * self.initial_scale * scale;
        let h = (self.y2 - self.y1).abs() * self.initial_scale * scale;
        (w.max(2.0), h.max(2.0))
    }
}

pub const DEFAULT_ICON_SYSTEM: IconSystem = IconSystem {
    found: false,
    x1: -100.0,
    y1: -100.0,
    x2: 100.0,
    y2: 100.0,
    preserve_aspect_ratio: true,
    initial_scale: 0.1,
};

/// Places by component name: (x, y, width, height).
pub type IconPlaces = HashMap<String, (f64, f64, f64, f64)>;

/// Places by component name: (x, y, width, height, rotation).
pub type LayoutPlaces = HashMap<String, [f64; 5]>;

pub struct Library {
    pub roots: Vec<String>,
    pub classes: Vec<ClassRef>,
    pub top: HashMap<String, Option<ClassId>>,
    /// Library versions asked for by `uses` annotations.
    pub uses: HashMap<String, String>,
    pub path_cache: HashMap<String, Option<ClassId>>,
    pub port_cache: HashMap<String, PortInfo>,
    pub icon_cache: HashMap<String, IconSystem>,
    pub place_cache: HashMap<String, Rc<IconPlaces>>,
    /// The files this run lays out (absolute paths; "" is a text without a file).
    pub targets: HashSet<String>,
    /// The sources of files read for the layout of a base class, and of the file being laid out.
    pub sources: HashMap<String, Option<Rc<Source>>>,
    /// Per class this run lays out: the place of its own components.
    pub layout_cache: HashMap<String, Option<Rc<LayoutPlaces>>>,
    pub warnings: Vec<String>,
    pub files_parsed: usize,
}

/// `a/b`, or `b` when it is absolute.
pub fn join_path(a: &str, b: &str) -> String {
    Path::new(a).join(b).to_string_lossy().into_owned()
}

pub fn is_file(p: &str) -> bool {
    Path::new(p).is_file()
}

pub fn is_dir(p: &str) -> bool {
    Path::new(p).is_dir()
}

/// The directory of a path, "" when it has none.
pub fn dir_name(p: &str) -> String {
    match Path::new(p).parent() {
        Some(d) => d.to_string_lossy().into_owned(),
        // The root is its own directory, as in Julia's `dirname`.
        None if Path::new(p).has_root() => p.to_owned(),
        None => String::new(),
    }
}

/// The absolute path of `p`, with `.` and `..` resolved by name (links are not followed).
pub fn abs_path(p: &str) -> String {
    let path = Path::new(p);
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(d) => d.join(path),
            Err(_) => path.to_path_buf(),
        }
    };
    let mut out = PathBuf::new();
    for c in full.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().into_owned()
}

/// The key of a file in `targets` and `sources`.
pub fn file_path(file: &str) -> String {
    if file.is_empty() { String::new() } else { abs_path(file) }
}

/// The contents of a file, or `None`. Bytes that are not UTF-8 become U+FFFD.
pub fn read_text(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    })
}

/// The parsed text, or the message of the parse error (`file:line:column: message`).
pub fn parse_text(text: &str, file_name: &str) -> Result<Absyn::Program, String> {
    let result = parser::parse(text, file_name, file_name, parser::Grammar::Modelica3, false, 0.0);
    // The messages are kept per thread: take them after every parse.
    let messages = parser::take_syntax_messages();
    match result {
        Ok(prog) => Ok(prog),
        Err(_) => {
            let first = messages.iter().find(|m| m.severity == parser::SyntaxSeverity::Error);
            Err(match first {
                Some(m) if m.line1 > 0 => format!("{file_name}:{}:{}: {}", m.line1, m.col1, m.message),
                Some(m) => format!("{file_name}: {}", m.message),
                None => format!("{file_name}: cannot parse"),
            })
        }
    }
}

/// The leading numbers of a version string: `4.1.0+maint.om` gives [4, 1, 0].
pub fn version_numbers(s: &str) -> Vec<i64> {
    let mut v = Vec::new();
    let mut cur: Option<i64> = None;
    for ch in s.chars() {
        if let Some(d) = ch.to_digit(10) {
            cur = Some(cur.unwrap_or(0).wrapping_mul(10).wrapping_add(i64::from(d)));
        } else if ch == '.'
            && let Some(n) = cur
        {
            v.push(n);
            cur = None;
        } else {
            break;
        }
    }
    v.extend(cur);
    v
}

fn collect_elements(out: &mut Vec<Ref<Absyn::Element>>, parts: &metamodelica::List<Ref<Absyn::ClassPart>>) {
    for part in parts {
        if let Absyn::PUBLIC { contents } | Absyn::PROTECTED { contents } = &**part {
            for item in contents {
                if let Absyn::ELEMENTITEM { element } = &**item {
                    out.push(element.clone());
                }
            }
        }
    }
}

impl Library {
    /// A library that looks for classes in `roots`, then in the directories of
    /// `OPENMODELICALIBRARY` and `MODELICAPATH`, then in `~/.openmodelica/libraries`.
    pub fn new(roots: &[String]) -> Library {
        let mut all: Vec<String> = roots.iter().filter(|r| is_dir(r)).cloned().collect();
        for var in ["OPENMODELICALIBRARY", "MODELICAPATH"] {
            let value = std::env::var(var).unwrap_or_default();
            for r in value.split(if cfg!(windows) { ';' } else { ':' }) {
                if !r.is_empty() && is_dir(r) {
                    all.push(r.to_owned());
                }
            }
        }
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            let om = join_path(&join_path(&home, ".openmodelica"), "libraries");
            if is_dir(&om) {
                all.push(om);
            }
        }
        Library {
            roots: all,
            classes: Vec::new(),
            top: HashMap::new(),
            uses: HashMap::new(),
            path_cache: HashMap::new(),
            port_cache: HashMap::new(),
            icon_cache: HashMap::new(),
            place_cache: HashMap::new(),
            targets: HashSet::new(),
            sources: HashMap::new(),
            layout_cache: HashMap::new(),
            warnings: Vec::new(),
            files_parsed: 0,
        }
    }

    /// Enter a class into the library.
    pub fn add_class(
        &mut self,
        class: Ref<Absyn::Class>,
        name: String,
        dir: String,
        parent: Option<ClassId>,
        file: String,
    ) -> ClassId {
        self.classes.push(ClassRef {
            class,
            name,
            dir,
            parent,
            file,
            elements: None,
            locals: HashMap::new(),
            locals_loaded: false,
        });
        self.classes.len() - 1
    }

    /// The classes of a file, empty when it cannot be read or parsed.
    pub fn parse_classes(&mut self, file: &str) -> Vec<Ref<Absyn::Class>> {
        let parsed = read_text(file).and_then(|text| parse_text(&text, file).ok());
        match parsed {
            Some(prog) => {
                self.files_parsed += 1;
                prog.classes.iter().cloned().collect()
            }
            None => {
                self.warnings.push(format!("cannot parse {file}"));
                Vec::new()
            }
        }
    }

    /// The class `name` of a file.
    pub fn class_of_file(&mut self, file: &str, name: &str) -> Option<Ref<Absyn::Class>> {
        self.parse_classes(file).into_iter().find(|c| c.name == name)
    }

    /// The public and protected elements of a class.
    pub fn elements_of(&mut self, c: ClassId) -> Rc<Vec<Ref<Absyn::Element>>> {
        if let Some(e) = &self.classes[c].elements {
            return e.clone();
        }
        let mut out = Vec::new();
        match &*self.classes[c].class.body {
            Absyn::PARTS { classParts: parts, .. } | Absyn::CLASS_EXTENDS { parts, .. } => {
                collect_elements(&mut out, parts)
            }
            _ => {}
        }
        let out = Rc::new(out);
        self.classes[c].elements = Some(out.clone());
        out
    }

    /// The top-level class `name`: `root/name/package.mo`, `root/name.mo`, or `root/name VERSION`.
    pub fn top_level(&mut self, name: &str) -> Option<ClassId> {
        if let Some(c) = self.top.get(name) {
            return *c;
        }
        self.top.insert(name.to_owned(), None);
        let mut found: Option<ClassId> = None;
        for root in self.roots.clone() {
            let dir = join_path(&root, name);
            let file = join_path(&dir, "package.mo");
            let single = join_path(&root, &format!("{name}.mo"));
            if is_file(&file) {
                found = self.top_from_file(&file, name, &dir);
            } else if is_file(&single) {
                found = self.top_from_file(&single, name, "");
            } else {
                let mut best = String::new();
                let mut best_v: Vec<i64> = Vec::new();
                let want = self.uses.get(name).cloned().unwrap_or_default();
                let mut entries: Vec<String> = match std::fs::read_dir(&root) {
                    Ok(rd) => rd
                        .flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect(),
                    Err(_) => Vec::new(),
                };
                // Julia's `readdir` is sorted; the first match and ties depend on the order.
                entries.sort();
                let prefix = format!("{name} ");
                for e in entries {
                    let Some(ver) = e.strip_prefix(&prefix) else {
                        continue;
                    };
                    let path = join_path(&root, &e);
                    if !(is_file(&join_path(&path, "package.mo")) || (e.ends_with(".mo") && is_file(&path))) {
                        continue;
                    }
                    if !want.is_empty() && ver.starts_with(&want) {
                        best = e;
                        break;
                    }
                    let vn = version_numbers(ver);
                    if best.is_empty() || best_v < vn {
                        best_v = vn;
                        best = e;
                    }
                }
                if !best.is_empty() {
                    let p = join_path(&root, &best);
                    found = if is_dir(&p) {
                        self.top_from_file(&join_path(&p, "package.mo"), name, &p)
                    } else {
                        self.top_from_file(&p, name, "")
                    };
                }
            }
            if found.is_some() {
                break;
            }
        }
        self.top.insert(name.to_owned(), found);
        found
    }

    fn top_from_file(&mut self, file: &str, name: &str, dir: &str) -> Option<ClassId> {
        let c = self.class_of_file(file, name)?;
        let id = self.add_class(c.clone(), name.to_owned(), dir.to_owned(), None, file.to_owned());
        self.note_uses(&c);
        Some(id)
    }

    /// Record the versions of `annotation(uses(Name(version = "...")))`; the first one seen wins.
    pub fn note_uses(&mut self, c: &Absyn::Class) {
        let Some(u) = find_modifier(&class_annotation_args(c), "uses") else {
            return;
        };
        for a in modifier_args(&u) {
            let Absyn::MODIFICATION { path, .. } = &*a else {
                continue;
            };
            let Absyn::IDENT { name } = &**path else {
                continue;
            };
            let Some(v) = find_modifier(&modifier_args(&a), "version") else {
                continue;
            };
            if let Some(e) = modifier_exp(&v)
                && let Absyn::STRING { value } = &*e
                && !self.uses.contains_key(name.as_str())
            {
                self.uses.insert(name.to_string(), value.to_string());
            }
        }
    }

    /// The class `name` declared in `c`: nested, or in a file of its directory.
    pub fn local_class(&mut self, c: ClassId, name: &str) -> Option<ClassId> {
        if !self.classes[c].locals_loaded {
            self.classes[c].locals_loaded = true;
            let elements = self.elements_of(c);
            for e in elements.iter() {
                let Absyn::ELEMENT { specification, .. } = &**e else {
                    continue;
                };
                if let Absyn::CLASSDEF { class_: cls, .. } = &**specification
                    && !self.classes[c].locals.contains_key(cls.name.as_str())
                {
                    let full = format!("{}.{}", self.classes[c].name, cls.name);
                    let file = self.classes[c].file.clone();
                    let id = self.add_class(cls.clone(), full, String::new(), Some(c), file);
                    self.classes[c].locals.insert(cls.name.to_string(), Some(id));
                }
            }
        }
        if let Some(r) = self.classes[c].locals.get(name) {
            return *r;
        }
        let mut found: Option<ClassId> = None;
        if !self.classes[c].dir.is_empty() {
            let cdir = self.classes[c].dir.clone();
            let full = format!("{}.{}", self.classes[c].name, name);
            let dir = join_path(&cdir, name);
            let pkg = join_path(&dir, "package.mo");
            let file = join_path(&cdir, &format!("{name}.mo"));
            if is_file(&pkg) {
                if let Some(a) = self.class_of_file(&pkg, name) {
                    found = Some(self.add_class(a, full, dir, Some(c), pkg));
                }
            } else if is_file(&file)
                && let Some(b) = self.class_of_file(&file, name)
            {
                found = Some(self.add_class(b, full, String::new(), Some(c), file));
            }
        }
        self.classes[c].locals.insert(name.to_owned(), found);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(version_numbers("4.1.0+maint.om"), vec![4, 1, 0]);
        assert_eq!(version_numbers("3.2.3"), vec![3, 2, 3]);
        assert_eq!(version_numbers("trunk"), Vec::<i64>::new());
        // Versions compare as slices: number by number, a prefix first.
        assert!([3, 2, 3][..] < [4, 0][..]);
        assert!([4][..] < [4, 1][..]);
    }

    #[test]
    fn parse_error_has_position() {
        let e = parse_text("model A Real x end A;", "a.mo").err().unwrap_or_default();
        assert!(e.starts_with("a.mo:1:"), "{e}");
    }

    #[test]
    fn nested_class() {
        let prog = parse_text("package P model A end A; model B end B; end P;", "").ok();
        let Some(prog) = prog else {
            panic!("the test text parses");
        };
        let mut lib = Library::new(&[]);
        let cls = prog.classes.iter().next().cloned();
        let Some(cls) = cls else {
            panic!("one class");
        };
        let p = lib.add_class(cls, "P".to_owned(), String::new(), None, String::new());
        let b = lib.local_class(p, "B");
        assert_eq!(b.map(|b| lib.classes[b].name.clone()), Some("P.B".to_owned()));
        assert_eq!(lib.local_class(p, "C"), None);
    }
}
