// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The program `mocoal`. The result goes to the standard output, everything else to the standard
//! error. The exit status is 0, 1 when a file could not be laid out, or 2 for a command line
//! that is wrong.
//! Mirrors `src/main.jl` of ModelicaComponentAutolayout.jl.

use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::api::{VERSION, layout_file_in, write_text};
use crate::cli::{CommandLine, USAGE_TEXT, modelica_files, parse_arguments, svg_file_name};
use crate::lookup::{Library, file_path, is_dir, is_file, join_path};

fn say(s: &str) {
    eprintln!("{s}");
}

/// The texts that `-i` writes when all files are done, so that every file is read as it was.
#[derive(Default)]
struct Pending {
    names: Vec<String>,
    texts: Vec<String>,
}

/// Lay out one file as the command line asks: (done, changed).
fn run_file(cl: &CommandLine, lib: &mut Library, file: &str, several: bool, pending: &mut Pending) -> (bool, bool) {
    let Ok(res) = catch_unwind(AssertUnwindSafe(|| layout_file_in(file, &cl.opts, lib))) else {
        say(&format!("mocoal: {file}: internal error, no layout"));
        return (false, false);
    };
    if !res.error.is_empty() {
        say(&format!("mocoal: {}", res.error));
        return (false, false);
    }
    if !cl.quiet {
        for w in &res.warnings {
            say(&format!("mocoal: warning: {w}"));
        }
        let mut placed = 0;
        let mut routed = 0;
        for c in &res.classes {
            placed += c.placed;
            routed += c.routed;
            if !several {
                say(&format!(
                    "{}: {} of {} components placed, {} of {} connections routed",
                    c.name, c.placed, c.components, c.routed, c.connections
                ));
            }
        }
        if several && !res.classes.is_empty() {
            say(&format!(
                "{file}: {} classes, {placed} components placed, {routed} connections routed",
                res.classes.len()
            ));
        }
    }
    for (name, svg) in res.svg_names.iter().zip(&res.svgs) {
        let path = join_path(&cl.svg_dir, &svg_file_name(name));
        if !write_text(&path, svg) {
            say(&format!("mocoal: cannot write {path}"));
            return (false, res.changed);
        }
    }
    if cl.dry_run {
        return (true, res.changed);
    }
    if cl.in_place {
        if res.changed {
            pending.names.push(file.to_owned());
            pending.texts.push(res.text);
        }
    } else if !cl.output.is_empty() {
        if !write_text(&cl.output, &res.text) {
            say(&format!("mocoal: cannot write {}", cl.output));
            return (false, res.changed);
        }
    } else if std::io::stdout().write_all(res.text.as_bytes()).is_err() {
        return (false, res.changed);
    }
    (true, res.changed)
}

/// Run `mocoal` with the command line `args` (without the program name); the exit status.
/// See `mocoal --help`.
pub fn run(args: &[String]) -> u8 {
    let cl = parse_arguments(args);
    if !cl.error.is_empty() {
        say(&format!("mocoal: {}", cl.error));
        say("try: mocoal --help");
        return 2;
    }
    if cl.help {
        print!("{USAGE_TEXT}");
        return 0;
    }
    if cl.version {
        println!("mocoal {VERSION}");
        return 0;
    }
    if cl.inputs.is_empty() {
        eprint!("{USAGE_TEXT}");
        return 2;
    }
    let mut files = Vec::new();
    let mut several = cl.inputs.len() > 1;
    for p in &cl.inputs {
        if is_dir(p) {
            several = true;
            modelica_files(&mut files, p);
        } else if is_file(p) {
            files.push(p.clone());
        } else {
            say(&format!("mocoal: no such file: {p}"));
            return 2;
        }
    }
    if several && !cl.in_place && !cl.dry_run {
        say("mocoal: several files: use -i to change them or -n to only report");
        return 2;
    }
    if !cl.output.is_empty() && (several || cl.in_place || cl.dry_run) {
        say("mocoal: -o takes one input file, without -i and -n");
        return 2;
    }
    if !cl.svg_dir.is_empty() && std::fs::create_dir_all(&cl.svg_dir).is_err() {
        say(&format!("mocoal: cannot create {}", cl.svg_dir));
        return 2;
    }
    let mut lib = Library::new(&cl.opts.libraries);
    for file in &files {
        lib.targets.insert(file_path(file));
    }
    let mut failed = 0;
    let mut changed = 0;
    let mut pending = Pending::default();
    for file in &files {
        let (ok, ch) = run_file(&cl, &mut lib, file, several, &mut pending);
        failed += usize::from(!ok);
        changed += usize::from(ch);
    }
    for (name, text) in pending.names.iter().zip(&pending.texts) {
        if !write_text(name, text) {
            say(&format!("mocoal: cannot write {name}"));
            failed += 1;
            changed -= 1;
        }
    }
    if several && !cl.quiet {
        let word = if cl.dry_run { "would change" } else { "changed" };
        say(&format!("{} files, {changed} {word}, {failed} failed", files.len()));
    }
    u8::from(failed > 0)
}
