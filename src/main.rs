// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The standalone program `mocoal`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    // Deep nesting in a model needs more stack than the main thread has.
    match mocoal::stack::on_large_stack(|| mocoal::program::run(&args)) {
        Ok(status) => ExitCode::from(status),
        Err(_) => {
            eprintln!("mocoal: internal error");
            ExitCode::from(1)
        }
    }
}
