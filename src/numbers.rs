// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! Number functions that Julia has built in, written out here so that both implementations
//! compute the same coordinates: `sind`, `cosd` (after Julia's `base/special/trig.jl`), the order
//! of `isless`, the text of a `Float64`, `tryparse`.

// The kernel coefficients are written as in Julia's source.
#![allow(clippy::excessive_precision)]

use std::cmp::Ordering;

/// `(hi, lo)`: the angle in radians, with the rounding error of the product.
fn deg2rad_ext(x: f64) -> (f64, f64) {
    let m = 0.017453292519943295;
    let m_hi = 0.01745329238474369;
    let m_lo = 1.3519960527851425e-10;
    let u = 134217729.0 * x;
    let x_hi = u - (u - x);
    let x_lo = x - x_hi;
    let y_hi = m * x;
    let y_lo = x_hi * m_lo + (x_lo * m_hi + ((x_hi * m_hi - y_hi) + x_lo * m_lo));
    (y_hi, y_lo)
}

// sin_kernel and cos_kernel: Julia's translation of openlibm's k_sin.c and k_cos.c, which are
// under this licence:
//
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunPro, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.

fn sin_kernel((hi, lo): (f64, f64)) -> f64 {
    const DS1: f64 = -1.66666666666666324348e-01;
    const DS2: f64 = 8.33333333332248946124e-03;
    const DS3: f64 = -1.98412698298579493134e-04;
    const DS4: f64 = 2.75573137070700676789e-06;
    const DS5: f64 = -2.50507602534068634195e-08;
    const DS6: f64 = 1.58969099521155010221e-10;
    let y2 = hi * hi;
    let y4 = y2 * y2;
    let r = y2.mul_add(y2.mul_add(DS4, DS3), DS2) + y2 * y4 * y2.mul_add(DS6, DS5);
    let y3 = y2 * hi;
    hi - ((y2 * (0.5 * lo - y3 * r) - lo) - y3 * DS1)
}

fn cos_kernel((hi, lo): (f64, f64)) -> f64 {
    const DC1: f64 = 4.16666666666666019037e-02;
    const DC2: f64 = -1.38888888888741095749e-03;
    const DC3: f64 = 2.48015872894767294178e-05;
    const DC4: f64 = -2.75573143513906633035e-07;
    const DC5: f64 = 2.08757232129817482790e-09;
    const DC6: f64 = -1.13596475577881948265e-11;
    let y2 = hi * hi;
    let y4 = y2 * y2;
    let r = y2 * y2.mul_add(y2.mul_add(DC3, DC2), DC1) + y4 * y4 * y2.mul_add(y2.mul_add(DC6, DC5), DC4);
    let half_y2 = 0.5 * y2;
    let w = 1.0 - half_y2;
    w + (((1.0 - w) - half_y2) + (y2 * r - hi * lo))
}

/// The sine of an angle in degrees; exact at the multiples of 30 and 90 where the sine is.
pub fn sind(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let rx = (x % 360.0).copysign(x);
    let arx = rx.abs();
    if rx == 0.0 {
        rx
    } else if arx < 45.0 {
        sin_kernel(deg2rad_ext(rx))
    } else if arx <= 135.0 {
        cos_kernel(deg2rad_ext(90.0 - arx)).copysign(rx)
    } else if arx == 180.0 {
        0.0_f64.copysign(rx)
    } else if arx < 225.0 {
        sin_kernel(deg2rad_ext((180.0 - arx) * rx.signum()))
    } else if arx <= 315.0 {
        -cos_kernel(deg2rad_ext(270.0 - arx)).copysign(rx)
    } else {
        sin_kernel(deg2rad_ext(rx - 360.0_f64.copysign(rx)))
    }
}

/// The cosine of an angle in degrees.
pub fn cosd(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let rx = (x % 360.0).abs();
    if rx <= 45.0 {
        cos_kernel(deg2rad_ext(rx))
    } else if rx < 135.0 {
        sin_kernel(deg2rad_ext(90.0 - rx))
    } else if rx <= 225.0 {
        -cos_kernel(deg2rad_ext(180.0 - rx))
    } else if rx < 315.0 {
        sin_kernel(deg2rad_ext(rx - 270.0))
    } else {
        cos_kernel(deg2rad_ext(360.0 - rx))
    }
}

/// The order of Julia's `isless`: -0.0 before 0.0, NaN last.
pub fn float_order(a: &f64, b: &f64) -> Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => match a.partial_cmp(b) {
            Some(Ordering::Equal) | None => b.is_sign_negative().cmp(&a.is_sign_negative()),
            Some(o) => o,
        },
    }
}

/// The permutation that sorts `v`, equal values in their order (Julia's `sortperm` with `MergeSort`).
pub fn sort_perm(v: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..v.len()).collect();
    order.sort_by(|&a, &b| float_order(&v[a], &v[b]));
    order
}

/// The text of a number as Julia's `string(::Float64)` writes it: the shortest digits that give the
/// number back, with an exponent below 1e-4 and from 1e6.
pub fn float_text(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_owned();
    } else if x.is_infinite() {
        return (if x < 0.0 { "-Inf" } else { "Inf" }).to_owned();
    }
    let sci = format!("{x:e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let e: i32 = exponent.parse().unwrap_or(0);
    if x == 0.0 || (-5 < e && e < 6) {
        let s = format!("{x}");
        if s.contains('.') { s } else { format!("{s}.0") }
    } else if mantissa.contains('.') {
        format!("{mantissa}e{e}")
    } else {
        format!("{mantissa}.0e{e}")
    }
}

/// A number from text, as Julia's `tryparse(Float64, s)`: white space around it is allowed.
pub fn parse_float(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;

    #[test]
    fn degrees() {
        assert_eq!(sind(30.0), 0.5);
        assert_eq!(cosd(60.0), 0.5);
        assert_eq!(cosd(90.0), 0.0);
        assert_eq!(sind(180.0), 0.0);
        assert_eq!(cosd(270.0), 0.0);
        assert_eq!(sind(-90.0), -1.0);
        // Values of Julia 1.13.
        assert_eq!(sind(45.0), 0.7071067811865476);
        assert_eq!(cosd(45.0), 0.7071067811865476);
        assert_eq!(sind(10.0), 0.17364817766693036);
        assert_eq!(cosd(10.0), 0.984807753012208);
    }

    #[test]
    fn text() {
        let cases = [
            (1.0e-5, "1.0e-5"),
            (1.5e-5, "1.5e-5"),
            (0.0001, "0.0001"),
            (0.01, "0.01"),
            (123456.5, "123456.5"),
            (1234567.5, "1.2345675e6"),
            (100000.0, "100000.0"),
            (1.0e6, "1.0e6"),
            (999999.25, "999999.25"),
            (1.5e15, "1.5e15"),
            (-1234567.25, "-1.23456725e6"),
            (1.0e21, "1.0e21"),
        ];
        for (x, s) in cases {
            assert_eq!(float_text(x), s);
        }
    }

    #[test]
    fn order() {
        assert_eq!(
            sort_perm(&[2.0, f64::NAN, 0.0, -0.0, 2.0, -1.0]),
            vec![5, 3, 2, 0, 4, 1]
        );
    }
}
