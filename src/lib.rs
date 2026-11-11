// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Gives the components of a Modelica model a `Placement` and its connections a `Line`.

// The Absyn of OpenModelica keeps its MetaModelica names.
#![allow(non_snake_case)]

// The public parts: the Rust API, its options, the program and the C interface.

mod absyn;
mod annotate;
pub mod api;
pub mod capi;
mod cli;
mod diagram;
mod ends;
mod extract;
mod graphics;
mod grid;
mod layered;
mod lookup;
mod numbers;
pub mod options;
mod placement;
pub mod program;
mod resolve;
mod routing;
mod spring;
mod svg;
mod tokens;
