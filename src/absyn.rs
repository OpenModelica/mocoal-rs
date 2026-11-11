// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Helpers for the Absyn tree: paths, modifiers, and the numbers in annotations.
//! Mirrors `src/absyn.jl` of ModelicaComponentAutolayout.jl.

use metamodelica::{List, Ref};
use openmodelica_ast::Absyn;

use crate::numbers::{cosd, parse_float, sind};

/// The arguments of an annotation or of a modifier.
pub type Args = Vec<Ref<Absyn::ElementArg>>;

/// The elements of an Absyn list as a vector.
pub fn list_vector<T: Clone>(l: &List<T>) -> Vec<T> {
    l.iter().cloned().collect()
}

/// The positions of `info`: (start line, start column, end line, end column).
pub fn info_position(info: &Absyn::Info) -> (i32, i32, i32, i32) {
    (
        info.lineNumberStart,
        info.columnNumberStart,
        info.lineNumberEnd,
        info.columnNumberEnd,
    )
}

/// The identifiers of a path.
pub fn path_parts(p: &Absyn::Path) -> Vec<String> {
    let mut parts = Vec::new();
    let mut q = p;
    loop {
        match q {
            Absyn::FULLYQUALIFIED { path } => q = path,
            Absyn::QUALIFIED { name, path } => {
                parts.push(name.to_string());
                q = path;
            }
            Absyn::IDENT { name } => {
                parts.push(name.to_string());
                return parts;
            }
        }
    }
}

/// The path of a type specification.
pub fn type_path(t: &Absyn::TypeSpec) -> &Absyn::Path {
    match t {
        Absyn::TPATH { path, .. } | Absyn::TCOMPLEX { path, .. } => path,
    }
}

/// The arguments of the annotation of an optional comment.
pub fn comment_args(c: Option<&Ref<Absyn::Comment>>) -> Args {
    match c.and_then(|c| c.annotation_.as_ref()) {
        Some(a) => list_vector(&a.elementArgs),
        None => Vec::new(),
    }
}

/// The arguments of the annotations of a class.
pub fn class_annotation_args(cls: &Absyn::Class) -> Args {
    let mut out = Vec::new();
    match &*cls.body {
        Absyn::PARTS { ann, .. } | Absyn::CLASS_EXTENDS { ann, .. } => {
            for a in ann {
                out.extend(a.elementArgs.iter().cloned());
            }
        }
        Absyn::DERIVED { comment, .. } | Absyn::ENUMERATION { comment, .. } => out = comment_args(comment.as_ref()),
        _ => {}
    }
    out
}

/// The modification `name` among the arguments.
pub fn find_modifier(args: &[Ref<Absyn::ElementArg>], name: &str) -> Option<Ref<Absyn::ElementArg>> {
    args.iter()
        .find(|a| matches!(&***a, Absyn::MODIFICATION { path, .. } if matches!(&**path, Absyn::IDENT { name: n } if n == name)))
        .cloned()
}

/// The arguments of a modification: the `a, b` of `name(a, b)`.
pub fn modifier_args(m: &Absyn::ElementArg) -> Args {
    match m {
        Absyn::MODIFICATION {
            modification: Some(mo), ..
        } => list_vector(&mo.elementArgLst),
        _ => Vec::new(),
    }
}

/// The value of a modification: the `e` of `name = e`.
pub fn modifier_exp(m: &Absyn::ElementArg) -> Option<Ref<Absyn::Exp>> {
    if let Absyn::MODIFICATION {
        modification: Some(mo), ..
    } = m
        && let Absyn::EQMOD { exp, .. } = &*mo.eqMod
    {
        return Some(exp.clone());
    }
    None
}

/// The modifier at `path` below the arguments.
pub fn find_modifier_path(args: &[Ref<Absyn::ElementArg>], path: &[&str]) -> Option<Ref<Absyn::ElementArg>> {
    let (first, rest) = path.split_first()?;
    let mut found = find_modifier(args, first)?;
    for name in rest {
        found = find_modifier(&modifier_args(&found), name)?;
    }
    Some(found)
}

/// The first argument of `DynamicSelect(a, b)`, or the expression itself.
pub fn static_exp(e: &Absyn::Exp) -> &Absyn::Exp {
    if let Absyn::CALL {
        function_,
        functionArgs: function_args,
        ..
    } = e
        && let Absyn::CREF_IDENT { name, .. } = &**function_
        && name == "DynamicSelect"
        && let Absyn::FUNCTIONARGS { args, .. } = &**function_args
        && let Some(first) = args.iter().next()
    {
        return static_exp(first);
    }
    e
}

/// The value of a numeric expression of literals, NaN when it has none.
pub fn eval_number(e: &Absyn::Exp) -> f64 {
    match static_exp(e) {
        Absyn::INTEGER { value } => f64::from(*value),
        Absyn::REAL { value } => parse_float(value).unwrap_or(f64::NAN),
        Absyn::UNARY { op, exp } => match op {
            Absyn::UMINUS => -eval_number(exp),
            Absyn::UPLUS => eval_number(exp),
            _ => f64::NAN,
        },
        Absyn::BINARY { exp1, op, exp2 } => {
            let a = eval_number(exp1);
            let b = eval_number(exp2);
            match op {
                Absyn::ADD => a + b,
                Absyn::SUB => a - b,
                Absyn::MUL => a * b,
                Absyn::DIV => a / b,
                _ => f64::NAN,
            }
        }
        _ => f64::NAN,
    }
}

/// The numbers of a (nested) array expression, in order.
pub fn flat_numbers(out: &mut Vec<f64>, e: &Absyn::Exp) {
    match static_exp(e) {
        Absyn::ARRAY { arrayExp: items } => {
            for x in items {
                flat_numbers(out, x);
            }
        }
        Absyn::MATRIX { matrix } => {
            for row in matrix {
                for x in row {
                    flat_numbers(out, x);
                }
            }
        }
        other => out.push(eval_number(other)),
    }
}

/// The numbers of the modifier `name = {...}`: empty when absent, or when one is not a literal.
pub fn modifier_numbers(args: &[Ref<Absyn::ElementArg>], name: &str) -> Vec<f64> {
    let Some(e) = find_modifier(args, name).and_then(|m| modifier_exp(&m)) else {
        return Vec::new();
    };
    let mut v = Vec::new();
    flat_numbers(&mut v, &e);
    if v.iter().any(|x| x.is_nan()) { Vec::new() } else { v }
}

/// The truth value of the modifier `name = true|false`, or `default`.
pub fn modifier_bool(args: &[Ref<Absyn::ElementArg>], name: &str, default: bool) -> bool {
    match find_modifier(args, name).and_then(|m| modifier_exp(&m)) {
        Some(e) => match static_exp(&e) {
            Absyn::BOOL { value } => *value,
            _ => default,
        },
        None => default,
    }
}

/// A number as written in an annotation: at most two decimals, no trailing zeros.
pub fn format_number(x: f64) -> String {
    if !x.is_finite() {
        return "0".to_owned();
    }
    let r = (x * 100.0).round_ties_even() / 100.0;
    if r == r.round_ties_even() && r.abs() < 1.0e15 {
        return format!("{}", r as i64);
    }
    crate::numbers::float_text(r)
}

/// A component's placement: its origin, extent and rotation. `has` is false without an extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transformation {
    pub has: bool,
    pub ox: f64,
    pub oy: f64,
    pub ex1: f64,
    pub ey1: f64,
    pub ex2: f64,
    pub ey2: f64,
    pub rotation: f64,
}

pub const NO_TRANSFORMATION: Transformation = Transformation {
    has: false,
    ox: 0.0,
    oy: 0.0,
    ex1: -10.0,
    ey1: -10.0,
    ex2: 10.0,
    ey2: 10.0,
    rotation: 0.0,
};

/// The transformation given by the arguments of `transformation(...)` or `iconTransformation(...)`.
pub fn read_transformation(args: &[Ref<Absyn::ElementArg>]) -> Transformation {
    let ext = modifier_numbers(args, "extent");
    if ext.len() != 4 {
        return NO_TRANSFORMATION;
    }
    let org = modifier_numbers(args, "origin");
    let rot = modifier_numbers(args, "rotation");
    let (ox, oy) = if org.len() == 2 { (org[0], org[1]) } else { (0.0, 0.0) };
    Transformation {
        has: true,
        ox,
        oy,
        ex1: ext[0],
        ey1: ext[1],
        ex2: ext[2],
        ey2: ext[3],
        rotation: if rot.len() == 1 { rot[0] } else { 0.0 },
    }
}

/// The point (x, y) rotated by `degrees` counter-clockwise about the origin.
pub fn rotate_point(x: f64, y: f64, degrees: f64) -> (f64, f64) {
    if degrees == 0.0 {
        return (x, y);
    }
    let c = cosd(degrees);
    let s = sind(degrees);
    (x * c - y * s, x * s + y * c)
}

/// The colour of the first shape with a `lineColor` in `Icon(graphics = {...})`, as `{r,g,b}`, or "".
pub fn icon_line_color(args: &[Ref<Absyn::ElementArg>]) -> String {
    let Some(e) = find_modifier_path(args, &["Icon", "graphics"]).and_then(|g| modifier_exp(&g)) else {
        return String::new();
    };
    let Absyn::ARRAY { arrayExp: shapes } = &*e else {
        return String::new();
    };
    for shape in shapes {
        let Absyn::CALL {
            functionArgs: function_args,
            ..
        } = &**shape
        else {
            continue;
        };
        let Absyn::FUNCTIONARGS { argNames: named, .. } = &**function_args else {
            continue;
        };
        for na in named {
            if na.argName == "lineColor" {
                let mut v = Vec::new();
                flat_numbers(&mut v, &na.argValue);
                if v.len() == 3 && v.iter().all(|x| !x.is_nan()) {
                    return format!(
                        "{{{},{},{}}}",
                        format_number(v[0]),
                        format_number(v[1]),
                        format_number(v[2])
                    );
                }
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(format_number(10.0), "10");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(0.125), "0.12");
        assert_eq!(format_number(0.375), "0.38");
        assert_eq!(format_number(-12.5), "-12.5");
        assert_eq!(format_number(f64::NAN), "0");
        assert_eq!(rotate_point(10.0, 0.0, 90.0), (0.0, 10.0));
        assert_eq!(rotate_point(10.0, 0.0, 180.0), (-10.0, 0.0));
    }
}
