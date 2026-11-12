// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the OSMC Public License (OSMC-PL) version 1.8 or
// the GNU AGPL version 3 license, according to the recipient's choice. See LICENSE.md.

//! The layout: the diagram of a model, the placement of its components and the routing of its lines.

pub mod diagram;
pub mod ends;
pub mod extract;
pub mod grid;
pub mod layered;
pub mod placement;
pub mod routing;
pub mod spring;
