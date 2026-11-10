// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The options of a layout. Mirrors `src/options.jl` of ModelicaComponentAutolayout.jl.

/// The options of a layout; the values are the words of the command line.
#[derive(Clone, Debug)]
pub struct Options {
    /// `layered`, `spring` or `grid`.
    pub layout: String,
    /// `orthogonal`, `straight`, `bezier` or `none`.
    pub route: String,
    /// `right` or `down`: the direction of the signal flow.
    pub direction: String,
    /// Horizontal and vertical space between components.
    pub dx: f64,
    pub dy: f64,
    /// Factor on the default component size.
    pub scale: f64,
    /// Keep existing placements and lines, lay out only what has none.
    pub keep: bool,
    /// Names of the classes to lay out; empty means all.
    pub classes: Vec<String>,
    /// Directories to search for libraries, before the default ones.
    pub libraries: Vec<String>,
    /// Also produce an SVG preview per class.
    pub svg: bool,
    /// Turn two-pin components along the flow.
    pub rotate: bool,
    /// What happens to the shapes of the Diagram layer: `move`, `keep` or `drop`.
    pub graphics: String,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            layout: "layered".to_owned(),
            route: "orthogonal".to_owned(),
            direction: "right".to_owned(),
            dx: 30.0,
            dy: 20.0,
            scale: 1.0,
            keep: false,
            classes: Vec::new(),
            libraries: Vec::new(),
            svg: false,
            rotate: true,
            graphics: "move".to_owned(),
        }
    }
}

impl Options {
    /// An error message for option values that are not known.
    pub fn check(&self) -> Result<(), String> {
        if !matches!(self.layout.as_str(), "layered" | "spring" | "grid") {
            Err(format!("unknown layout: {}", self.layout))
        } else if !matches!(self.route.as_str(), "orthogonal" | "straight" | "bezier" | "none") {
            Err(format!("unknown route: {}", self.route))
        } else if !matches!(self.direction.as_str(), "right" | "down") {
            Err(format!("unknown direction: {}", self.direction))
        } else if !(self.dx.is_finite() && self.dy.is_finite() && self.scale.is_finite()) {
            Err("spacing and scale must be finite numbers".to_owned())
        } else if self.dx < 0.0 || self.dy < 0.0 {
            Err("spacing must not be negative".to_owned())
        } else if self.scale <= 0.0 {
            Err("scale must be positive".to_owned())
        } else if !matches!(self.graphics.as_str(), "move" | "keep" | "drop") {
            Err(format!("unknown graphics: {}", self.graphics))
        } else {
            Ok(())
        }
    }
}
