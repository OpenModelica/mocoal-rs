// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Gives the components of a Modelica model a `Placement` and its connections a `Line`.

// The Absyn of OpenModelica keeps its MetaModelica names.
#![allow(non_snake_case)]

pub mod absyn;
pub mod annotate;
pub mod api;
pub mod capi;
pub mod cli;
pub mod diagram;
pub mod ends;
pub mod extract;
pub mod graphics;
pub mod grid;
pub mod layered;
pub mod lookup;
pub mod numbers;
pub mod options;
pub mod placement;
pub mod program;
pub mod resolve;
pub mod routing;
pub mod spring;
pub mod svg;
pub mod tokens;
