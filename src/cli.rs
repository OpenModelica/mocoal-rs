// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The command line of `mocoal`, and the entry of the shared library, which takes its options as
//! the same command-line text.
//! Mirrors `src/cli.jl` of ModelicaComponentAutolayout.jl.

use crate::api::layout_string;
use crate::lookup::{is_dir, is_file, join_path};
use crate::numbers::parse_float;
use crate::options::Options;

pub const USAGE_TEXT: &str = "usage: mocoal [options] file.mo|directory ...

Give the components of Modelica models a placement and the connections a line.
One file without -o, -i or -n: the result is printed.

  -o, --output FILE      write the result to FILE (one input file)
  -i, --in-place         change the input files
  -n, --dry-run          report only, write nothing
  -c, --class NAME       lay out only this class or package (may be repeated)
  -k, --keep             keep existing placements and lines, add what is missing
      --layout NAME      layered (default), spring, grid
      --route NAME       orthogonal (default), straight, bezier, none
      --direction NAME   right (default), down
      --spacing DX[,DY]  space between components (30,20)
      --scale S          factor on the component size (1)
      --no-rotate        do not turn two-pin components
      --graphics NAME    shapes of the Diagram layer: move (default), keep, drop
  -L, --library DIR      a directory with libraries (may be repeated)
      --svg DIR          write an SVG preview of every diagram into DIR
  -q, --quiet            no report
  -h, --help             this text
      --version          the version
";

/// A parsed command line.
#[derive(Clone, Debug, Default)]
pub struct CommandLine {
    pub opts: Options,
    pub inputs: Vec<String>,
    pub output: String,
    pub in_place: bool,
    pub dry_run: bool,
    pub quiet: bool,
    pub svg_dir: String,
    pub help: bool,
    pub version: bool,
    /// Why the command line cannot be understood, or "".
    pub error: String,
}

impl CommandLine {
    /// Set the option `name` that takes the value `value`. False when there is no such option.
    fn set_value_option(&mut self, name: &str, value: &str) -> bool {
        let o = &mut self.opts;
        match name {
            "-o" | "--output" => self.output = value.to_owned(),
            "-c" | "--class" => o.classes.push(value.to_owned()),
            "-L" | "--library" => o.libraries.push(value.to_owned()),
            "--layout" => o.layout = value.to_owned(),
            "--route" => o.route = value.to_owned(),
            "--direction" => o.direction = value.to_owned(),
            "--graphics" => o.graphics = value.to_owned(),
            "--svg" => {
                self.svg_dir = value.to_owned();
                o.svg = true;
            }
            "--scale" => match parse_float(value) {
                Some(s) => o.scale = s,
                None => self.error = format!("--scale: not a number: {value}"),
            },
            "--spacing" => {
                let parts: Vec<&str> = value.split(',').collect();
                let dx = parse_float(parts[0]);
                let dy = if parts.len() == 2 { parse_float(parts[1]) } else { None };
                match dx {
                    Some(dx) if parts.len() == 1 || (parts.len() == 2 && dy.is_some()) => {
                        o.dx = dx;
                        if let Some(dy) = dy {
                            o.dy = dy;
                        }
                    }
                    _ => self.error = format!("--spacing: expected DX or DX,DY: {value}"),
                }
            }
            _ => return false,
        }
        true
    }

    /// Set the option `name` that takes no value. False when there is no such option.
    fn set_flag(&mut self, name: &str) -> bool {
        match name {
            "-i" | "--in-place" => self.in_place = true,
            "-n" | "--dry-run" => self.dry_run = true,
            "-k" | "--keep" => self.opts.keep = true,
            "--no-rotate" => self.opts.rotate = false,
            "-q" | "--quiet" => self.quiet = true,
            "-h" | "--help" => self.help = true,
            "--version" => self.version = true,
            _ => return false,
        }
        true
    }
}

fn takes_value(name: &str) -> bool {
    matches!(
        name,
        "-o" | "--output"
            | "-c"
            | "--class"
            | "-L"
            | "--library"
            | "--layout"
            | "--route"
            | "--direction"
            | "--svg"
            | "--scale"
            | "--spacing"
            | "--graphics"
    )
}

/// The command line `args`; `error` is set when it cannot be understood.
pub fn parse_arguments(args: &[String]) -> CommandLine {
    let mut cl = CommandLine::default();
    let mut i = 0;
    let mut only_inputs = false;
    while i < args.len() && cl.error.is_empty() {
        let a = args[i].as_str();
        i += 1;
        if only_inputs || !a.starts_with('-') || a == "-" {
            cl.inputs.push(a.to_owned());
            continue;
        }
        if a == "--" {
            only_inputs = true;
            continue;
        }
        let (name, given) = match a.find('=').filter(|_| a.starts_with("--")) {
            Some(eq) => (&a[..eq], Some(&a[eq + 1..])),
            None => (a, None),
        };
        if takes_value(name) {
            let value = match given {
                Some(v) => v,
                None => {
                    let Some(v) = args.get(i) else {
                        cl.error = format!("{name}: a value is missing");
                        break;
                    };
                    i += 1;
                    v.as_str()
                }
            };
            cl.set_value_option(name, value);
        } else if given.is_some() || !cl.set_flag(name) {
            cl.error = format!("unknown option: {a}");
        }
    }
    if cl.error.is_empty()
        && let Err(msg) = cl.opts.check()
    {
        cl.error = msg;
    }
    cl
}

/// White space as Julia's `isspace` has it.
fn is_space(c: char) -> bool {
    c.is_whitespace() && c != '\u{2028}' && c != '\u{2029}'
}

/// The words of the command-line text `s`. A word may be in double quotes (`-L "/my libs"`),
/// which keep the spaces in it.
pub fn split_arguments(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in s.chars() {
        if c == '"' {
            quoted = !quoted;
            started = true;
        } else if is_space(c) && !quoted {
            if started {
                out.push(std::mem::take(&mut cur));
                started = false;
            }
        } else {
            cur.push(c);
            started = true;
        }
    }
    if started {
        out.push(cur);
    }
    out
}

/// The entry of the shared library. `options` is command-line text (`"--route straight
/// --keep"`); options that name files are not allowed. Returns the new text and the warnings,
/// one per line, or the error when there is no layout.
pub fn layout_text(text: &str, file_name: &str, options: &str) -> Result<(String, String), String> {
    let cl = parse_arguments(&split_arguments(options));
    if !cl.error.is_empty() {
        return Err(cl.error);
    }
    if !cl.inputs.is_empty()
        || !cl.output.is_empty()
        || cl.in_place
        || !cl.svg_dir.is_empty()
        || cl.dry_run
        || cl.quiet
        || cl.help
        || cl.version
    {
        return Err(format!("only layout options are allowed here: {options}"));
    }
    let res = layout_string(text, file_name, &cl.opts);
    if !res.error.is_empty() {
        return Err(res.error);
    }
    Ok((res.text, res.warnings.join("\n")))
}

/// The `.mo` files in `dir` and below, sorted.
pub fn modelica_files(out: &mut Vec<String>, dir: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in &names {
        if name.starts_with('.') {
            continue;
        }
        let path = join_path(dir, name);
        if is_dir(&path) {
            modelica_files(out, &path);
        } else if name.ends_with(".mo") && is_file(&path) {
            out.push(path);
        }
    }
}

/// The file name of the SVG preview of a class.
pub fn svg_file_name(class_name: &str) -> String {
    let keep = |c: char| c.is_alphabetic() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-');
    let mut s: String = class_name.chars().map(|c| if keep(c) { c } else { '_' }).collect();
    s.push_str(".svg");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        split_arguments(s)
    }

    #[test]
    fn arguments() {
        assert_eq!(words("  --route straight  -k "), ["--route", "straight", "-k"]);
        assert_eq!(words("-L \"/my libs\" -c \"\""), ["-L", "/my libs", "-c", ""]);
        let cl = parse_arguments(&words(
            "--spacing=40,25 --scale 2 -c A --class=B.C --no-rotate x.mo -- -y.mo",
        ));
        assert_eq!(cl.error, "");
        assert_eq!(
            (cl.opts.dx, cl.opts.dy, cl.opts.scale, cl.opts.rotate),
            (40.0, 25.0, 2.0, false)
        );
        assert_eq!(cl.opts.classes, ["A", "B.C"]);
        assert_eq!(cl.inputs, ["x.mo", "-y.mo"]);
        assert_eq!(parse_arguments(&words("--scale")).error, "--scale: a value is missing");
        assert_eq!(parse_arguments(&words("--scale x")).error, "--scale: not a number: x");
        assert_eq!(
            parse_arguments(&words("--spacing 1,2,3")).error,
            "--spacing: expected DX or DX,DY: 1,2,3"
        );
        assert_eq!(
            parse_arguments(&words("--spacing 1,")).error,
            "--spacing: expected DX or DX,DY: 1,"
        );
        assert_eq!(parse_arguments(&words("--frob")).error, "unknown option: --frob");
        assert_eq!(parse_arguments(&words("--keep=1")).error, "unknown option: --keep=1");
        assert_eq!(
            parse_arguments(&words("--layout circle")).error,
            "unknown layout: circle"
        );
    }

    #[test]
    fn library_entry() {
        assert_eq!(
            layout_text("", "", "--graphics move"),
            Ok((String::new(), String::new()))
        );
        assert_eq!(
            layout_text("", "", "-q"),
            Err("only layout options are allowed here: -q".to_owned())
        );
        assert_eq!(svg_file_name("A.B c/d"), "A.B_c_d.svg");
    }
}
