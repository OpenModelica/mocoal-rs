// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the OSMC Public License (OSMC-PL) version 1.8 or
// the GNU AGPL version 3 license, according to the recipient's choice. See LICENSE.md.

//! Gives the components of a Modelica model a `Placement` and its connections a `Line`.

// The Absyn of OpenModelica keeps its MetaModelica names.
#![allow(non_snake_case)]

// The public parts: the Rust API, its options, the program and the C interface.
pub mod api;
pub mod capi;
pub mod options;
pub mod program;
pub mod stack;

mod cli;
mod layout;
mod numbers;
mod source;
mod write;

#[cfg(test)]
mod tests;
