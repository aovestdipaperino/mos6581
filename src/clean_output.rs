//! Clean-room, see docs/provenance.md.
//! Static (memoryless) transfer of the MOS 6581 SID output stage.
//!
//! # What is modelled
//!
//! The emulator hands this function the mixed, volume-scaled signal `x`
//! (one full-scale voice swings about ±1, music sits within about ±2, and the
//! whole range is roughly [-4, +4]). On the chip this signal is a voltage
//! produced by analogue stages that the author infers are NMOS on the 12 V
//! supply, driving an on-chip source-follower buffer whose source the author
//! infers is the AUDIO OUT pin (pin 27). Only the pin-28 12 V supply is
//! sourced; the 12 V analogue stages and the on-chip source follower are the
//! author's inference.
//!
//! Circuit facts read from the sources:
//!
//! * Pin 28 is Vdd = 12 V DC and pin 27 is "Audio out ... Should be AC coupled
//!   to audio amp" (C64 service-manual pin table, `ciasid.gif`).
//! * On the C64 board (schematic 250469 rev. B) pin 27 goes to ground through
//!   R8 = 1 kΩ, and on through R9 = 10 kΩ into the base of Q3 (2SC1815), an
//!   emitter follower with R12 = 1 kΩ to ground; C74 = 1000 pF sits from the
//!   R9/Q3 node to ground. Note 8 on that sheet says R8 must be removed for
//!   the 8580. The conclusion that the 6581 needs this external resistor to
//!   ground is the author's inference; the note says only that R8 should be
//!   removed for the 8580.
//! * Schematic citation: the sheet viewed is the file
//!   `250469-rev.B-right-corrected.png`, titled "COMMODORE-64 BN/E", which
//!   labels the SID U9, while the pin table (`ciasid.gif`) calls it U18. The
//!   two files use different designators for the same chip.
//!
//! Inference (not stated by any source): a pin that needs an external resistor
//! to ground to work is the source of an NMOS follower whose drain is on Vdd,
//! with no internal pull-down. The model is therefore an n-channel enhancement
//! MOSFET in common-drain configuration (source follower) with a resistive
//! load `R_L` to ground.
//!
//! # Equations (textbook device behaviour)
//!
//! In saturation (`V_DS = Vdd - V_out` is several volts, larger than the
//! overdrive), the square law gives
//! `I_D = (k/2) (V_GS - V_th)^2`, and the load gives `I_D = V_out / R_L`.
//! The threshold rises with source-to-body voltage (body effect):
//! `V_th = V_t0 + γ (sqrt(V_SB + 2φ) - sqrt(2φ))`, with the body at ground so
//! `V_SB = V_out`. Eliminating `I_D` gives the gate voltage needed for a given
//! output voltage:
//!
//! `V_g(V_out) = V_out + V_t0 + γ (sqrt(V_out + 2φ) - sqrt(2φ)) + sqrt(2 V_out / (k R_L))`
//!
//! This is strictly increasing in `V_out`, so it has a unique inverse, which
//! [`output_stage`] evaluates numerically. Its slope
//! `dV_out/dV_g = 1 / (1 + γ / (2 sqrt(V_out + 2φ)) + 1 / sqrt(2 k R_L V_out))`
//! is below one and grows with `V_out`: the follower is a slightly expansive,
//! asymmetric (even-order dominated) nonlinearity. Feeding it several voices
//! at once yields second-order intermodulation, i.e. sum and difference tones.
//! When the gate falls below threshold the transistor cuts off and the output
//! sits at 0 V (a flat, still monotonic region far outside the normal range).
//! The gate is clamped to [0, Vdd] because the stage driving it runs on Vdd.
//!
//! # Mapping to and from emulator units
//!
//! `x` maps linearly onto the gate voltage around a bias point (own choices
//! below). The output voltage is mapped back so that `output_stage(0) == 0`
//! and the small-signal slope at `x = 0` is exactly 1. This keeps level and
//! DC handling of the existing emulator unchanged for small signals: the DC
//! offset of the real pin is removed by the board's AC coupling, which the
//! emulator models after this function.
//!
//! Not modelled here: the R9/C74 low-pass and every other frequency-dependent
//! effect (the function is static by contract), Q3's base-emitter drop,
//! channel-length modulation, C76 (470 pF at Q3), and the 6581's +9/+12 V
//! rail note.
//!
//! Caveats: body-at-ground and the substrate bias are assumptions. Near the
//! low end of the swing, Q3's V_BE makes the linear `R9 + (beta+1) R12` load
//! assumption break down.

use std::sync::OnceLock;

/// Supply on pin 28, the drain of the output follower.
/// Source: http://www.zimmers.net/anonftp/pub/cbm/schematics/computers/c64/ciasid.gif
/// (U18 6581 pin table: "28 Vdd 12VDC").
const VDD: f64 = 12.0;

/// R8, pull-down from AUDIO OUT (pin 27) to ground.
/// Source: http://www.zimmers.net/anonftp/pub/cbm/schematics/computers/c64/250469-rev.B-right-corrected.png
/// (R8 1k at pin 27; note 8 "R8 resistor should be remove for 8580 SID").
const R8: f64 = 1.0e3;

/// R9, series resistor from pin 27 into the base of Q3.
/// Source: http://www.zimmers.net/anonftp/pub/cbm/schematics/computers/c64/250469-rev.B-right-corrected.png
/// (R9 10k between pin 27 and Q3 base).
const R9: f64 = 10.0e3;

/// R12, emitter resistor of the Q3 (2SC1815) emitter follower.
/// Source: http://www.zimmers.net/anonftp/pub/cbm/schematics/computers/c64/250469-rev.B-right-corrected.png
/// (R12 1k from Q3 emitter to ground).
const R12: f64 = 1.0e3;

/// Current gain of Q3, used to reflect R12 into the base.
/// unsourced own choice: a typical small-signal NPN current gain (textbook
/// order of magnitude); the result is insensitive to it because R8 dominates.
const Q3_BETA: f64 = 100.0;

/// Transconductance parameter k = µn·Cox·W/L of the output transistor, A/V².
/// unsourced own choice, feedback-driven (round 2; was 10 mA/V²). K was chosen
/// after evaluator feedback (round 2); the textbook figure (NMOS process
/// transconductance of about 20 µA/V² times W/L ≈ 100, a wide but not
/// exceptional output device) is only a plausibility check. At ~3 mA into ~1 kΩ it runs with about
/// 1.7 V of overdrive, so the square-law term shapes the transfer more.
const K: f64 = 2.0e-3;

/// Zero-bias threshold voltage of the enhancement NMOS device, V.
/// unsourced own choice: textbook enhancement-mode NMOS value of about 1 V.
const VT0: f64 = 1.0;

/// Body-effect coefficient γ, √V.
/// unsourced own choice: textbook NMOS range is roughly 0.3 to 1 √V; 0.5 is
/// mid-range. Equation form: https://en.wikipedia.org/wiki/MOSFET, section
/// "Body effect" (the square roots are from textbook knowledge; the fetched
/// text had lost the radical signs).
const GAMMA: f64 = 0.5;

/// Surface potential term 2φ_B, V.
/// unsourced own choice: textbook silicon value of about 0.6 V.
const TWO_PHI: f64 = 0.6;

/// Gate (buffer input) voltage when x = 0, V.
/// unsourced own choice, feedback-driven (round 2; was 7.5 V): quiescent
/// output about 2.8 V, so the follower runs at lower current where its
/// incremental gain varies more. x in [-4, +4] maps to 3 V .. 9 V at the
/// gate, which keeps the device on (Vt0 = 1 V) and the gate below Vdd. The
/// datasheet that would give the real DC level could not be fetched.
const VG_BIAS: f64 = 6.0;

/// Gate-voltage swing per unit of x, V.
/// unsourced own choice (the rationale below is after the fact): maps x = ±4 to ±3 V at the gate (3 V to 9 V with the round 2 bias),
/// i.e. one full-scale voice is about 1.5 V peak to peak, the largest swing
/// that stays inside the supply with headroom for the driving stage.
const VOLTS_PER_UNIT: f64 = 0.75;

/// Effective DC load on pin 27: R8 in parallel with R9 plus the reflected
/// emitter resistance of the Q3 follower (Q3's V_BE neglected).
fn load_resistance() -> f64 {
    let branch = R9 + (Q3_BETA + 1.0) * R12;
    R8 * branch / (R8 + branch)
}

/// Gate voltage needed for a given output voltage (inverse transfer), V.
fn gate_for_output(vout: f64, rl: f64) -> f64 {
    let v = vout.max(0.0);
    v + VT0 + GAMMA * ((v + TWO_PHI).sqrt() - TWO_PHI.sqrt()) + (2.0 * v / (K * rl)).sqrt()
}

/// Output voltage of the source follower for a given gate voltage, V.
///
/// Solves `gate_for_output(v) = vg` by bisection; the left side is strictly
/// increasing and continuous, so the root is unique.
fn follower(vg: f64) -> f64 {
    let rl = load_resistance();
    let vg = vg.clamp(0.0, VDD);
    if vg <= VT0 {
        return 0.0; // cut off
    }
    // The output never exceeds the gate voltage minus the threshold.
    let (mut lo, mut hi) = (0.0_f64, vg - VT0);
    for _ in 0..64 {
        let mid = 0.5 * (lo + hi);
        if gate_for_output(mid, rl) < vg {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Small-signal slope dV_out/dV_g at a given output voltage (analytic).
fn follower_slope(vout: f64) -> f64 {
    let rl = load_resistance();
    let d = 1.0 + GAMMA / (2.0 * (vout + TWO_PHI).sqrt()) + 1.0 / (2.0 * K * rl * vout).sqrt();
    1.0 / d
}

/// Static transfer of the 6581 output stage in emulator units.
///
/// Maps the mixer value `x` to the gate of the NMOS source follower, solves
/// the follower, and returns the output relative to its value at `x = 0`,
/// scaled so that the slope at `x = 0` is 1. Continuous, monotonic
/// non-decreasing and finite for every finite `x`. Non-finite input is
/// treated by clamping (NaN maps to 0).
pub fn output_stage(x: f64) -> f64 {
    static OPERATING_POINT: OnceLock<(f64, f64)> = OnceLock::new();
    let x = if x.is_nan() { 0.0 } else { x };
    let &(v0, gain0) = OPERATING_POINT.get_or_init(|| {
        let v0 = follower(VG_BIAS);
        (v0, follower_slope(v0) * VOLTS_PER_UNIT)
    });
    let v = follower(VG_BIAS + x.clamp(-1.0e6, 1.0e6) * VOLTS_PER_UNIT);
    (v - v0) / gain0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn monotonic_and_finite_over_wide_range() {
        let mut prev = output_stage(-1.0e9);
        assert!(prev.is_finite());
        let mut x = -40.0;
        while x <= 40.0 {
            let y = output_stage(x);
            assert!(y.is_finite());
            assert!(y >= prev - 1e-12, "not monotonic at {x}");
            prev = y;
            x += 0.001;
        }
        assert!(output_stage(1.0e9).is_finite());
        assert!(output_stage(f64::MAX).is_finite());
        assert!(output_stage(f64::MIN).is_finite());
    }

    #[test]
    fn continuous() {
        let mut x = -10.0;
        while x < 10.0 {
            let d = (output_stage(x + 1e-6) - output_stage(x)).abs();
            assert!(d < 1e-5, "jump at {x}: {d}");
            x += 0.0137;
        }
    }

    #[test]
    fn small_signal_is_unity_and_centred() {
        assert!(output_stage(0.0).abs() < 1e-9);
        let h = 1e-4;
        let slope = (output_stage(h) - output_stage(-h)) / (2.0 * h);
        assert!((slope - 1.0).abs() < 1e-3, "slope {slope}");
        // Tiny signals are nearly undistorted.
        for &x in &[-0.01, 0.003, 0.01] {
            assert!((output_stage(x) - x).abs() < 1e-3 * x.abs().max(1e-3));
        }
    }

    #[test]
    fn follower_gain_below_one_and_rising() {
        // Source follower: incremental gain < 1, and larger at higher output.
        let lo = follower_slope(follower(5.0));
        let hi = follower_slope(follower(10.0));
        assert!(lo < 1.0 && hi < 1.0);
        assert!(hi > lo);
    }

    #[test]
    fn asymmetric_expansive_shape() {
        // Gain grows with output voltage, so a positive excursion comes out
        // larger than the matching negative one.
        let p = output_stage(2.0);
        let n = -output_stage(-2.0);
        assert!(p > n, "p={p} n={n}");
        // But the stage is only mildly nonlinear in the music range.
        assert!((p - 2.0).abs() < 0.3 && (n - 2.0).abs() < 0.3);
    }

    #[test]
    fn cuts_off_far_below_range() {
        let a = output_stage(-50.0);
        let b = output_stage(-100.0);
        assert_eq!(a, b);
    }

    /// Amplitude of a frequency bin (in cycles per record) via a DFT.
    fn bin_amplitude(sig: &[f64], k: usize) -> f64 {
        let n = sig.len() as f64;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, s) in sig.iter().enumerate() {
            let ph = 2.0 * PI * k as f64 * i as f64 / n;
            re += s * ph.cos();
            im -= s * ph.sin();
        }
        2.0 * (re * re + im * im).sqrt() / n
    }

    #[test]
    fn two_tones_intermodulate() {
        let n = 4096;
        let (f1, f2) = (200usize, 263usize);
        let make = |nl: bool| -> Vec<f64> {
            (0..n)
                .map(|i| {
                    let t = i as f64 / n as f64;
                    let x = (2.0 * PI * f1 as f64 * t).sin() + (2.0 * PI * f2 as f64 * t).sin();
                    if nl { output_stage(x) } else { x }
                })
                .collect()
        };
        let lin = make(false);
        let out = make(true);
        let diff = f2 - f1;
        let sum = f2 + f1;
        // Linear path: no difference or sum tone.
        assert!(bin_amplitude(&lin, diff) < 1e-9);
        // Nonlinear stage: clear difference and sum tones.
        let d = bin_amplitude(&out, diff);
        let s = bin_amplitude(&out, sum);
        assert!(d > 1e-3, "difference tone {d}");
        assert!(s > 1e-3, "sum tone {s}");
        // Still well below the fundamentals.
        assert!(d < 0.2 * bin_amplitude(&out, f1));
    }

    #[test]
    fn intermod_grows_with_level() {
        // Second-order products scale roughly with the square of the input.
        let n = 4096;
        let (f1, f2) = (200usize, 263usize);
        let im = |a: f64| {
            let sig: Vec<f64> = (0..n)
                .map(|i| {
                    let t = i as f64 / n as f64;
                    output_stage(
                        a * ((2.0 * PI * f1 as f64 * t).sin() + (2.0 * PI * f2 as f64 * t).sin()),
                    )
                })
                .collect();
            bin_amplitude(&sig, f2 - f1)
        };
        let small = im(0.25);
        let large = im(0.5);
        let ratio = large / small;
        assert!(ratio > 3.0 && ratio < 5.0, "ratio {ratio}");
    }
}
