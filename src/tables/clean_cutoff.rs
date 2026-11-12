//! Clean-room, see docs/provenance.md.
//! Clean-room physical model of the MOS 6581 SID filter cutoff frequency
//! as a function of its 11-bit cutoff register, for a SID fitted in a
//! Commodore 64.
//!
//! # Model
//!
//! The SID filter is a state-variable filter built from two integrators.
//! Bob Yannes (interview) says the cutoff is set by "FETs as voltage-controlled
//! resistors", driven by "an 11-bit D/A converter". Each integrator has time
//! constant `R*C`, so the centre frequency is
//!
//! ```text
//! f = 1 / (2 * pi * R * C) = G / (2 * pi * C)
//! ```
//!
//! where `G = 1/R` is the total integrator conductance and `C` is the
//! external filter capacitor (470 pF in the C64, from schematic 326106).
//!
//! The chain from register to frequency is:
//!
//! 1. **DAC.** The 11-bit value `fc` (0..=2047) maps linearly onto a gate
//!    voltage `Vg = V_DAC_MIN + (V_DAC_MAX - V_DAC_MIN) * fc / 2047`. The
//!    patent calls the control linear; the voltage range is an own choice.
//! 2. **Threshold with body effect.** The transistor's source sits at the
//!    integrator's bias node `V_SOURCE`, above the grounded substrate, so the
//!    threshold rises by the textbook body effect:
//!    `Vth = VTH0 + GAMMA * (sqrt(Vsb + 2phiF) - sqrt(2phiF))`.
//! 3. **Channel conductance.** In the triode region,
//!    `I_D = k * ((Vgs - Vth) * Vds - Vds^2 / 2)`, so the conductance at small
//!    `Vds` is `k * (Vgs - Vth)`: linear in overdrive. Below threshold the
//!    current falls exponentially as `exp(Vov / (n * VT))`. The two regimes are
//!    joined by `g = k * n*VT * ln(1 + exp(Vov / (n*VT)))`, which tends to
//!    `k * Vov` well above threshold and to `k * n*VT * exp(Vov / (n*VT))` well
//!    below.
//! 4. **Signal swing.** The audio signal across the FET is not small. The
//!    `-Vds^2/2` term makes the effective overdrive `Vgs - Vth - Vds/2`, so over
//!    a signal swing the device sees a spread of overdrives. The conductance is
//!    averaged over a uniform spread of `+-V_SWING_HALF`. This rounds the knee
//!    at threshold over a few hundred register steps.
//! 5. **Closed-filter floor.** With the FET off the filter still has a finite
//!    corner (the datasheet formula quoted in the patent is nonzero at
//!    `FCn = 0`). It is modelled as a fixed conductance in parallel with the FET.
//! 6. **Scale.** `k` is set so the corner at `fc = 2047` equals the upper end
//!    of the range the patent states for the filter.
//!
//! The result is nearly flat at the floor for the low codes (FET off), bends
//! smoothly through threshold, and then rises linearly with `fc`.

use std::f64::consts::PI;

/// Filter capacitor fitted to CAP1A/CAP1B (C10) and CAP2A/CAP2B (C11), in farads.
/// Source: http://www.zimmers.net/anonftp/pub/cbm/schematics/computers/c64/326106-2of2.gif
/// (schematic #326106 sheet 2 of 2, U18 6581 SID: "C10 470pf POLY", "C11 470pf POLY").
pub const C64_FILTER_CAP_F: f64 = 470e-12;

/// Corner frequency at `fc = 2047` for the C64 fit, Hz.
/// Source: https://patents.google.com/patent/US4677890A/en (registers 15-16:
/// "the approximate range of the filter ... is 30 Hz-12 kHz"). Applying that
/// upper end to the C64's 470 pF part is an unsourced own choice: Commodore
/// fitted a capacitor 4.7x smaller than the recommended 2200 pF, which only
/// keeps the range in the audio band if the real FETs conduct less than the
/// nominal formula says (Yannes: FET resistance "varied considerably with
/// processing").
const F_TOP_HZ: f64 = 12_000.0;

/// Corner frequency with the FET fully off (closed filter), Hz.
/// unsourced own choice: residual conduction (leakage and a FET gate only
/// slightly below threshold) leaves the closed corner at a few hundred Hz;
/// 250 Hz was picked after the round-1 evaluation reported that 140 Hz was
/// not within 1.5x of the reference.
const F_CLOSED_HZ: f64 = 250.0;

/// Largest register value (11 bits).
/// Source: https://patents.google.com/patent/US4677890A/en ("an 11-bit number
/// which linearly controls the cutoff"); https://www.oxyron.de/html/registers_sid.html
/// ($D415 FC2-FC0, $D416 FC10-FC3).
const FC_MAX: f64 = 2047.0;

/// SID drain supply VDD, volts.
/// Source: http://www.zimmers.net/anonftp/pub/cbm/schematics/computers/c64/ciasid.gif
/// (U18 6581 pin table: "28 Vdd 12VDC").
const VDD: f64 = 12.0;

/// DAC output (FET gate voltage) at `fc = 0`, volts.
/// unsourced own choice: placed 0.8 V below the FET's turn-on gate voltage so
/// the lowest codes are off, as a design margin against the process spread in
/// FET behaviour that Yannes describes.
const V_DAC_MIN: f64 = 6.0;

/// Margin of the DAC top below VDD, volts.
/// unsourced own choice: an NMOS output stage (source follower) cannot pull
/// higher than about one threshold below its 12 V drain supply.
const V_DAC_HEADROOM: f64 = 1.0;

/// DAC output (FET gate voltage) at `fc = 2047`, volts.
/// Derived: `VDD` (sourced) minus `V_DAC_HEADROOM` (own choice).
const V_DAC_MAX: f64 = VDD - V_DAC_HEADROOM;

/// DC level of the integrator node the FET source sits on, volts.
/// unsourced own choice: NMOS op-amps on a 12 V supply are biased a few volts
/// above ground to keep their input devices on; 5 V is a mid-range choice.
const V_SOURCE: f64 = 5.0;

/// Zero-bias NMOS enhancement threshold, volts.
/// unsourced own choice: typical for depletion-load NMOS processes of the late
/// 1970s (roughly 0.7 to 1.2 V).
const VTH0: f64 = 1.0;

/// Body-effect coefficient, sqrt(volts).
/// unsourced own choice: typical textbook value for a lightly doped NMOS substrate.
const GAMMA: f64 = 0.5;

/// Surface potential term 2*phiF, volts.
/// unsourced own choice: standard textbook value for silicon substrates.
const TWO_PHI_F: f64 = 0.6;

/// Subthreshold slope factor n = 1 + Cdep/Cox (dimensionless).
/// unsourced own choice: typical value between 1 and 2 for bulk MOSFETs.
const SLOPE_N: f64 = 1.5;

/// Half-width of the effective-overdrive spread caused by the signal swing, volts.
/// unsourced own choice: a 1 V peak-to-peak swing at the integrator, a moderate
/// fraction of what NMOS op-amps on a 12 V rail can swing; the `-Vds^2/2`
/// triode term shifts overdrive by `Vds/2`.
const V_SWING_HALF: f64 = 0.5;

/// Number of points used to average conductance over the swing.
/// unsourced own choice: enough for a smooth curve, cheap to evaluate.
const SWING_POINTS: usize = 32;

/// Boltzmann constant, J/K. Physical constant (CODATA exact value).
const K_BOLTZMANN: f64 = 1.380_649e-23;

/// Elementary charge, C. Physical constant (CODATA exact value).
const Q_ELECTRON: f64 = 1.602_176_634e-19;

/// Die temperature, kelvin.
/// unsourced own choice: about 27 C, a nominal room-temperature operating point.
const TEMPERATURE_K: f64 = 300.0;

/// Thermal voltage kT/q, volts.
/// Source: https://en.wikipedia.org/wiki/MOSFET (section "Cutoff, subthreshold,
/// and weak-inversion mode": "VT=kT/q is the thermal voltage").
fn thermal_voltage() -> f64 {
    K_BOLTZMANN * TEMPERATURE_K / Q_ELECTRON
}

/// Threshold voltage including body effect for source-to-bulk voltage `vsb`.
/// Source: https://en.wikipedia.org/wiki/MOSFET (section "Body effect").
fn threshold(vsb: f64) -> f64 {
    VTH0 + GAMMA * ((vsb + TWO_PHI_F).sqrt() - TWO_PHI_F.sqrt())
}

/// Normalised channel conductance `g / k` for overdrive `vov`, in volts.
///
/// Above threshold it tends to `vov` (triode small-signal conductance);
/// below threshold it decays as `n*VT*exp(vov / (n*VT))`.
/// Source: https://en.wikipedia.org/wiki/MOSFET (sections "Triode mode or
/// linear region" and "Cutoff, subthreshold, and weak-inversion mode"); the
/// softplus join between them is an unsourced own choice that is continuous,
/// positive and monotonic.
fn channel_conductance_per_k(vov: f64) -> f64 {
    let s = SLOPE_N * thermal_voltage();
    let x = vov / s;
    // Numerically stable softplus: ln(1 + e^x).
    let softplus = if x > 0.0 {
        x + (-x).exp().ln_1p()
    } else {
        x.exp().ln_1p()
    };
    s * softplus
}

/// `g / k` averaged over a uniform overdrive spread of `+-V_SWING_HALF`
/// (midpoint rule). Each term is increasing in `vov`, so the mean is too.
fn swing_averaged_conductance_per_k(vov: f64) -> f64 {
    let n = SWING_POINTS as f64;
    (0..SWING_POINTS)
        .map(|i| {
            let u = (i as f64 + 0.5) / n * 2.0 - 1.0;
            channel_conductance_per_k(vov + u * V_SWING_HALF)
        })
        .sum::<f64>()
        / n
}

/// FET gate overdrive (Vgs - Vth) for register value `fc`, in volts.
fn overdrive(fc: f64) -> f64 {
    let vg = V_DAC_MIN + (V_DAC_MAX - V_DAC_MIN) * fc / FC_MAX;
    vg - V_SOURCE - threshold(V_SOURCE)
}

/// Total integrator conductance in siemens for register value `fc` (clamped
/// to 0..=2047), for the C64 capacitor.
///
/// `G = G_floor + k * g(Vov)`, with `G_floor = 2*pi*C*F_CLOSED_HZ` and `k` set
/// so that `G(2047) = 2*pi*C*F_TOP_HZ`.
fn conductance(fc: f64) -> f64 {
    let fc = fc.clamp(0.0, FC_MAX);
    let g_floor = 2.0 * PI * C64_FILTER_CAP_F * F_CLOSED_HZ;
    let g_fet_full = 2.0 * PI * C64_FILTER_CAP_F * (F_TOP_HZ - F_CLOSED_HZ);
    let k = g_fet_full / swing_averaged_conductance_per_k(overdrive(FC_MAX));
    g_floor + k * swing_averaged_conductance_per_k(overdrive(fc))
}

/// Filter centre frequency in Hz for register value `fc` with filter
/// capacitor `cap_farads`, keeping the chip's internal conductances fixed.
/// Values of `fc` above 2047 are clamped.
///
/// # Panics
///
/// Panics if `cap_farads` is not finite and positive.
pub fn cutoff_hz_6581_with_cap(fc: u16, cap_farads: f64) -> f64 {
    assert!(
        cap_farads.is_finite() && cap_farads > 0.0,
        "capacitance must be finite and positive"
    );
    conductance(f64::from(fc)) / (2.0 * PI * cap_farads)
}

/// Filter centre frequency in Hz of a C64-fitted 6581 for the 11-bit cutoff
/// register value `fc` (0..=2047; larger values are clamped).
pub fn cutoff_hz_6581(fc: u16) -> f64 {
    cutoff_hz_6581_with_cap(fc, C64_FILTER_CAP_F)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Register steps per volt of gate voltage.
    fn codes_per_volt() -> f64 {
        FC_MAX / (V_DAC_MAX - V_DAC_MIN)
    }

    /// Register code at which the FET reaches threshold (Vov = 0).
    fn threshold_code() -> f64 {
        let v_on = V_SOURCE + threshold(V_SOURCE);
        (v_on - V_DAC_MIN) * codes_per_volt()
    }

    /// Code below which every point of the swing is below threshold.
    fn fully_off_code() -> f64 {
        threshold_code() - V_SWING_HALF * codes_per_volt()
    }

    /// Code above which every point of the swing is well into triode.
    fn fully_on_code() -> f64 {
        threshold_code() + V_SWING_HALF * codes_per_volt()
    }

    #[test]
    fn finite_positive_and_non_decreasing() {
        let mut prev = 0.0;
        for fc in 0..=2047u16 {
            let f = cutoff_hz_6581(fc);
            assert!(f.is_finite() && f > 0.0, "fc={fc} f={f}");
            assert!(f >= prev, "not monotonic at fc={fc}: {prev} -> {f}");
            prev = f;
        }
    }

    #[test]
    fn strictly_rising_once_fet_conducts() {
        let start = threshold_code().ceil() as u16;
        for fc in start..2047 {
            assert!(cutoff_hz_6581(fc + 1) > cutoff_hz_6581(fc), "fc={fc}");
        }
    }

    #[test]
    fn flat_while_fet_is_off() {
        let f0 = cutoff_hz_6581(0);
        let off = (fully_off_code() - 20.0) as u16;
        let rel = (cutoff_hz_6581(off) - f0) / f0;
        assert!(rel < 1e-2, "rel change {rel} while FET off");
    }

    #[test]
    fn subthreshold_contribution_is_exponential() {
        // While the whole swing is below threshold, the FET part grows by a
        // constant factor per equal step in gate voltage.
        let floor = F_CLOSED_HZ;
        let top = fully_off_code() - 40.0;
        let codes = [top - 60.0, top - 30.0, top];
        let fet: Vec<f64> = codes
            .iter()
            .map(|&c| cutoff_hz_6581(c as u16) - floor)
            .collect();
        assert!(fet.iter().all(|&x| x > 0.0), "{fet:?}");
        let r1 = fet[1] / fet[0];
        let r2 = fet[2] / fet[1];
        assert!(r1 > 2.0, "ratio {r1} not a strong exponential rise");
        assert!((r1 / r2 - 1.0).abs() < 0.05, "ratios {r1} vs {r2}");
    }

    #[test]
    fn linear_once_fully_in_triode() {
        let a = fully_on_code().ceil() + 50.0;
        let b = (a + FC_MAX) / 2.0;
        let c = FC_MAX;
        let fa = cutoff_hz_6581(a as u16);
        let fb = cutoff_hz_6581(b as u16);
        let fcv = cutoff_hz_6581(c as u16);
        let s1 = (fb - fa) / (b.floor() - a.floor());
        let s2 = (fcv - fb) / (c - b.floor());
        assert!((s1 / s2 - 1.0).abs() < 1e-2, "slopes {s1} vs {s2}");
    }

    #[test]
    fn knee_is_rounded_by_signal_swing() {
        // The slope ramps up across the knee rather than jumping.
        let th = threshold_code();
        let slope = |c: f64| cutoff_hz_6581(c as u16 + 5) - cutoff_hz_6581(c as u16);
        let below = slope(fully_off_code() + 20.0);
        let mid = slope(th);
        let above = slope(fully_on_code() + 20.0);
        assert!(below < mid && mid < above, "{below} {mid} {above}");
        assert!(above > 100.0 * slope(fully_off_code() - 60.0));
    }

    #[test]
    fn frequency_scales_inversely_with_capacitance() {
        for fc in [0u16, 300, 1024, 2047] {
            let f1 = cutoff_hz_6581_with_cap(fc, 470e-12);
            let f2 = cutoff_hz_6581_with_cap(fc, 940e-12);
            let f3 = cutoff_hz_6581_with_cap(fc, 2200e-12);
            assert!((f1 / f2 - 2.0).abs() < 1e-12);
            assert!((f1 * 470.0 / (f3 * 2200.0) - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn default_uses_c64_capacitor() {
        for fc in [0u16, 777, 2047] {
            assert_eq!(
                cutoff_hz_6581(fc),
                cutoff_hz_6581_with_cap(fc, C64_FILTER_CAP_F)
            );
        }
    }

    #[test]
    fn out_of_range_register_is_clamped() {
        assert_eq!(cutoff_hz_6581(4095), cutoff_hz_6581(2047));
        assert_eq!(cutoff_hz_6581(u16::MAX), cutoff_hz_6581(2047));
    }

    #[test]
    fn higher_body_bias_raises_threshold() {
        assert!(threshold(5.0) > threshold(0.0));
        assert!((threshold(0.0) - VTH0).abs() < 1e-12);
    }

    #[test]
    fn triode_conductance_tends_to_overdrive() {
        assert!((channel_conductance_per_k(3.0) / 3.0 - 1.0).abs() < 1e-6);
        assert!(channel_conductance_per_k(-1.0) < 1e-9);
        // Averaging a linear function over a symmetric spread changes nothing.
        assert!((swing_averaged_conductance_per_k(3.0) / 3.0 - 1.0).abs() < 1e-6);
    }
}
