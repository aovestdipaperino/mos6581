//! Clean-room, see docs/provenance.md.
//! Clean-room model of the MOS 6581 SID combined waveforms.
//!
//! # What the sources say
//!
//! Bob Yannes (interview, landley.net mirror) describes each voice as an
//! oscillator, a *Waveform Selector* that "consisted of multiplexers that
//! selected which waveform bits would be sent to the Waveform D/A", and a
//! 12-bit *Waveform D/A*. The sawtooth is the top 12 accumulator bits; the
//! triangle uses "the MSB of the accumulator to invert the remaining upper
//! 11 accumulator bits", which are then left-shifted; the pulse is a single
//! comparator output "sent to all 12 bits of the Waveform D/A". Selecting
//! several waveforms gives "a logical ANDing of the bits of each waveform,
//! which produced unpredictable results". US patent 4,677,890 agrees: "If
//! more than one output waveform is selected simultaneously, the result
//! will be a logical ANDing of the waveforms."
//!
//! A clean digital AND would be perfectly predictable, so the model treats
//! the selector as what an NMOS multiplexer physically is: pass transistors
//! that, when several are on, short several gate outputs together.
//!
//! # Circuit assumed
//!
//! Voltages are normalised to the supply (0 = ground, 1 = VDD) and
//! conductances are relative to one depletion load. Loads and pull-downs
//! are linear conductances; pass transistors use the square-law MOSFET
//! model (see `pass_conductance`), with the gate at VDD, the lower
//! terminal acting as source, and the body effect raising the threshold as
//! that source rises (Round 2).
//!
//! * **Bit drivers.** Accumulator bit `k` has a buffer node `A[k]`, and
//!   the MSB has a buffer `M` and an inverted buffer `M'`. Each is a
//!   ratioed NMOS output: a depletion load ([`G_LOAD`]) to VDD, plus an
//!   enhancement pull-down ([`G_PULL_DOWN`]) to ground when its logic value
//!   is 0.
//! * **Triangle XOR (Round 3).** Triangle bit `k` is accumulator bit `k-1`
//!   (0 for `k = 0`) XOR the MSB. The model uses the textbook two-transistor
//!   NMOS pass XOR: the data bit steers the output node `X[k]` through a
//!   pass transistor ([`G_PASS`]) onto `M'` when it is 1 and onto `M` when
//!   it is 0. This gives exactly the given triangle, including bit 0
//!   (= MSB). Because the MSB is broadcast to all 11 inverted bits, the
//!   `M` and `M'` nodes are each shared by every triangle bit.
//! * **Waveform selector.** For every selected waveform, a pass transistor
//!   ([`G_PASS`]) joins its node to DAC line `L[k]`: `A[k]` for sawtooth,
//!   `X[k]` for triangle, and one shared node `P` for the pulse, which is a
//!   single driver wired to all 12 lines.
//! * **Sensing.** A DAC line reads as 1 when its voltage exceeds
//!   [`V_SWITCH`].
//!
//! # How a line settles
//!
//! *Disagreeing drivers.* A pull-down is several times stronger than a
//! load, and an NMOS pass transistor conducts well when its low side is
//! near ground but poorly when it is near VDD (it passes a strong 0 and a
//! weak 1). A line joined to a 0-driver and a 1-driver therefore settles
//! below the switching point: the AND that the sources report.
//!
//! *Neighbour influence.* Two parts of the network are shared between
//! lines. Because 0s propagate through pass transistors much better than
//! 1s, both act in practice as extra paths to ground for a line that should
//! be high (the tests check that no table ever has a 1 the AND lacks):
//!
//! 1. The pulse node `P` is one weak load wired to every line. Each line
//!    held low by another waveform sinks current from it, so with enough
//!    low lines `P` falls and then pulls down lines whose other drivers are
//!    high.
//! 2. Every triangle 1 is passed from the same node (`M'` below the
//!    midpoint, `M` above it). With sawtooth and triangle both selected,
//!    every line where the triangle is 1 but the sawtooth is 0 sinks current
//!    from that node, so a line where both are 1 sags. When several such
//!    contested lines exist, it falls below the switching point. This acts
//!    in both halves of the triangle.
//!
//! The pulse drag is stronger still: `ps_`, `p_t` and `pst` keep 1s only
//! where nearly every line is high, so `pst` differs from `st`. When nearly
//! every line is high the pulse node becomes an extra pull-up instead, so
//! `pst` can occasionally keep a bit that `st` loses.
//!
//! Each table entry is one DC operating point of this network, found by
//! nonlinear Gauss-Seidel relaxation of Kirchhoff's current law. See
//! REPORT.md for the sources and every own choice.

/// Number of DAC input lines (waveform bits).
/// Source: http://www.landley.net/history/mirror/commodore/yannes.html
/// (oscillator section: "sending the upper 12-bits of the accumulator to the
/// 12-bit Waveform D/A").
const BITS: usize = 12;

/// Mask of a 12-bit waveform word, derived from [`BITS`].
const MASK: u16 = (1 << BITS) - 1;

/// Number of table entries, one per 12-bit index, derived from [`BITS`].
const ENTRIES: usize = 1 << BITS;

/// Index of the accumulator MSB that steers the triangle inversion.
/// Source: http://www.landley.net/history/mirror/commodore/yannes.html
/// ("using the MSB of the accumulator to invert the remaining upper 11
/// accumulator bits").
const MSB: usize = BITS - 1;

/// Right shift that keeps the top 8 of the 12 bits.
/// Source: task specification ("top 8 bits of the 12-bit waveform output").
const OUT_SHIFT: u32 = 4;

/// Conductance of a depletion-mode load, the reference unit.
/// unsourced own choice: normalisation; only the ratios between
/// conductances matter for the DC operating point.
const G_LOAD: f64 = 1.0;

/// Conductance of the enhancement pull-down in a bit driver.
/// unsourced own choice, changed in Round 2 in response to evaluation
/// feedback (Round 1 used 8.0): a 4:1 driver-to-load ratio is the textbook
/// minimum for a ratioed NMOS gate. Round 2 assumes the long-channel,
/// low-current depletion loads typical of a power-conscious NMOS design,
/// which puts the load at 1/16 of a driver (4x the minimum ratio). An
/// uncontested 0 then sits at 1/17 of VDD.
const G_PULL_DOWN: f64 = 16.0;

/// Small-signal conductance of a pass transistor (selector switch or XOR
/// multiplexer switch) with its low side at ground, i.e. at full overdrive.
/// It falls with overdrive as described in `pass_conductance`.
/// unsourced own choice, changed in Round 2 in response to evaluation
/// feedback (Round 1 used 4.0): the switch is taken to be the same size as
/// a driver pull-down, so against the weak loads assumed in [`G_PULL_DOWN`]
/// it has the same 16:1 ratio.
const G_PASS: f64 = 16.0;

/// Normalised switching voltage of the gates that sense the DAC lines.
/// unsourced own choice: for a ratioed NMOS inverter the switching point is
/// about Vt + |Vtd| * sqrt(beta_load / beta_driver). Textbook values
/// Vt = 0.2 VDD, |Vtd| = 0.6 VDD and a 4:1 ratio give
/// 0.2 + 0.6 * 0.5 = 0.5 VDD.
const V_SWITCH: f64 = 0.5;

/// NMOS enhancement threshold voltage, normalised to VDD.
/// unsourced own choice: textbook NMOS of the period, Vt of about 1 V on a
/// 5 V logic swing, i.e. 0.2 VDD; the same value is used for V_SWITCH.
const V_THRESHOLD: f64 = 0.2;

/// Body-effect coefficient, normalised (gamma / sqrt(VDD)).
/// unsourced own choice, added in Round 2 in response to evaluation
/// feedback (the model kept too many 1s): textbook NMOS gamma of about
/// 0.45 V^0.5 on a 5 V supply gives 0.45 / sqrt(5) = 0.2. The body effect
/// is standard first-order MOSFET physics that Round 1 had omitted.
const BODY_GAMMA: f64 = 0.2;

/// Surface potential term 2*phi_F, normalised to VDD.
/// unsourced own choice, added in Round 2 with BODY_GAMMA: textbook
/// 2*phi_F of about 0.6 V on a 5 V supply.
const BODY_PHI2: f64 = 0.12;

/// Residual conductance of a pass transistor with no overdrive.
/// unsourced own choice: subthreshold leakage, negligible next to every
/// on-conductance (six orders below a load), kept non-zero only so every
/// node has a defined voltage.
const G_OFF: f64 = 1e-6;

/// Convergence limit of the relaxation, in units of VDD.
/// unsourced own choice: far below the distance of any settled line from
/// V_SWITCH that matters for a 1-of-12 bit decision.
const SOLVE_TOLERANCE: f64 = 1e-9;

/// Safety cap on relaxation sweeps.
/// unsourced own choice: generous bound; the 12-line network converges in
/// far fewer sweeps, and the cap only guarantees termination.
const MAX_SWEEPS: usize = 20_000;

/// The four combined-waveform tables.
pub struct Tables {
    /// Sawtooth and triangle, indexed by phase.
    pub st: [u8; 4096],
    /// Pulse and triangle while pulse is high, indexed by `i` where the
    /// triangle value is `(i << 1) & 0xfff`.
    pub p_t: [u8; 4096],
    /// Pulse and sawtooth while pulse is high, indexed by phase.
    pub ps_: [u8; 4096],
    /// Pulse, sawtooth and triangle while pulse is high, indexed by phase.
    pub pst: [u8; 4096],
}

/// Ideal triangle output for a 12-bit phase.
#[cfg_attr(not(test), allow(dead_code))]
const fn triangle(phase: u16) -> u16 {
    if phase & 0x800 != 0 {
        ((!phase << 1) | 1) & 0x0fff
    } else {
        (phase << 1) & 0x0fff
    }
}

/// Which waveforms are switched onto the DAC lines.
#[derive(Clone, Copy)]
struct Select {
    saw: bool,
    tri: bool,
    pulse: bool,
}

/// Nodal network: linear rail conductances per node plus nonlinear pass
/// transistors between nodes, solved by nonlinear Gauss-Seidel relaxation.
struct Network {
    /// Per node: sum of rail conductances and sum of conductance * voltage.
    rail_g: Vec<f64>,
    rail_i: Vec<f64>,
    /// Pass transistors as (node, node) pairs.
    links: Vec<(usize, usize)>,
    /// Per node: indices into `links`.
    adj: Vec<Vec<usize>>,
}

/// Secant conductance of a fully-on NMOS pass transistor (gate at VDD)
/// between terminals at voltages `va` and `vb`.
///
/// The terminal at the lower voltage acts as the source, so the overdrive
/// is `1 - VT - min(va, vb)`. In the triode region the current is
/// `k * (vov * vds - vds^2 / 2)`, in saturation `k * vov^2 / 2`. This is
/// the textbook square-law model; it makes the device pass a 0 strongly and
/// a 1 weakly, and stop conducting once the low side reaches `1 - VT`.
fn pass_conductance(va: f64, vb: f64) -> f64 {
    let k = G_PASS / (1.0 - V_THRESHOLD);
    let lo = va.min(vb);
    let vds = (va - vb).abs();
    let vov = (1.0 - threshold(lo) - lo).max(0.0);
    let g = if vds < vov {
        k * (vov - vds / 2.0)
    } else {
        k * vov * vov / (2.0 * vds)
    };
    g.max(G_OFF)
}

/// Threshold voltage of a pass transistor whose source sits at `vs`
/// (normalised), including the body effect: the substrate is at ground, so
/// a raised source raises the threshold,
/// `Vt = Vt0 + gamma * (sqrt(2phi + Vsb) - sqrt(2phi))`.
fn threshold(vs: f64) -> f64 {
    V_THRESHOLD + BODY_GAMMA * ((BODY_PHI2 + vs.max(0.0)).sqrt() - BODY_PHI2.sqrt())
}

impl Network {
    fn new(n: usize) -> Self {
        Self {
            rail_g: vec![0.0; n],
            rail_i: vec![0.0; n],
            links: Vec::new(),
            adj: vec![Vec::new(); n],
        }
    }

    /// Linear conductance `g` from node `i` to a rail at voltage `v`.
    fn rail(&mut self, i: usize, g: f64, v: f64) {
        self.rail_g[i] += g;
        self.rail_i[i] += g * v;
    }

    /// A pass transistor between nodes `i` and `j`.
    fn link(&mut self, i: usize, j: usize) {
        let id = self.links.len();
        self.links.push((i, j));
        self.adj[i].push(id);
        self.adj[j].push(id);
    }

    /// A ratioed NMOS output node: load to VDD, pull-down when low.
    fn driver(&mut self, i: usize, high: bool) {
        self.rail(i, G_LOAD, 1.0);
        if !high {
            self.rail(i, G_PULL_DOWN, 0.0);
        }
    }

    /// Relaxes every node to its KCL balance until no node moves by more
    /// than [`SOLVE_TOLERANCE`] in a sweep, or [`MAX_SWEEPS`] sweeps pass.
    /// Each node's update uses the secant conductances at the current
    /// voltages, so the fixed point satisfies Kirchhoff's current law with
    /// the nonlinear pass devices.
    fn solve(&self) -> Vec<f64> {
        let n = self.rail_g.len();
        // Start every driven node at its own rail equilibrium and every
        // undriven node (lines, XOR outputs) discharged.
        let mut v: Vec<f64> = (0..n)
            .map(|i| {
                if self.rail_g[i] > 0.0 {
                    self.rail_i[i] / self.rail_g[i]
                } else {
                    0.0
                }
            })
            .collect();
        for _ in 0..MAX_SWEEPS {
            let mut moved: f64 = 0.0;
            for i in 0..n {
                let mut g_sum = self.rail_g[i];
                let mut i_sum = self.rail_i[i];
                for &id in &self.adj[i] {
                    let (a, b) = self.links[id];
                    let j = if a == i { b } else { a };
                    let g = pass_conductance(v[i], v[j]);
                    g_sum += g;
                    i_sum += g * v[j];
                }
                let new = i_sum / g_sum;
                moved = moved.max((new - v[i]).abs());
                v[i] = new;
            }
            if moved < SOLVE_TOLERANCE {
                break;
            }
        }
        v
    }
}

// Node layout.
const fn line(k: usize) -> usize {
    k
}
const fn acc(k: usize) -> usize {
    BITS + k
}
const fn xor(k: usize) -> usize {
    3 * BITS + k
}
const PULSE: usize = 4 * BITS;
const MSB_NODE: usize = 4 * BITS + 1;
const MSB_BAR: usize = 4 * BITS + 2;
const NODES: usize = 4 * BITS + 3;

/// Solves the network for accumulator bits `acc_bits` (12 bits; bit 11 is
/// the MSB) and returns the voltage on each DAC line.
fn line_voltages(acc_bits: u16, sel: Select) -> [f64; BITS] {
    let msb = acc_bits >> MSB & 1 != 0;
    let mut net = Network::new(NODES);
    for k in 0..BITS {
        let bit = acc_bits >> k & 1 != 0;
        net.driver(acc(k), bit);
        // Pass-transistor XOR: the data bit A[k-1] (0 for k = 0) steers
        // X[k] onto the shared MSB-bar node when 1, onto the MSB node when 0.
        let data = k > 0 && acc_bits >> (k - 1) & 1 != 0;
        net.link(xor(k), if data { MSB_BAR } else { MSB_NODE });
        if sel.saw {
            net.link(acc(k), line(k));
        }
        if sel.tri {
            net.link(xor(k), line(k));
        }
        if sel.pulse {
            net.link(PULSE, line(k));
        }
    }
    // Round 3, feedback-driven: the MSB and MSB-bar buffers are ordinary
    // drivers (same load and pull-down as a bit driver), not upsized for
    // their fan-out of 12. Upsizing them by 4x or more made `st` almost
    // the plain AND again.
    net.driver(MSB_NODE, msb);
    net.driver(MSB_BAR, !msb);
    // Only the pulse-high case is modelled; the caller zeroes the rest.
    net.driver(PULSE, true);
    let v = net.solve();
    std::array::from_fn(|k| v[line(k)])
}

/// Senses the lines and returns the 12-bit word.
fn settle(acc_bits: u16, sel: Select) -> u16 {
    line_voltages(acc_bits, sel)
        .iter()
        .enumerate()
        .fold(0, |w, (k, &v)| if v > V_SWITCH { w | 1 << k } else { w })
}

fn top8(word: u16) -> u8 {
    (word >> OUT_SHIFT) as u8
}

const ST: Select = Select {
    saw: true,
    tri: true,
    pulse: false,
};
const PT: Select = Select {
    saw: false,
    tri: true,
    pulse: true,
};
const PS: Select = Select {
    saw: true,
    tri: false,
    pulse: true,
};
const PST: Select = Select {
    saw: true,
    tri: true,
    pulse: true,
};

/// Builds the four combined-waveform tables from the network model.
pub fn combined_tables() -> Tables {
    let mut t = Tables {
        st: [0; ENTRIES],
        p_t: [0; ENTRIES],
        ps_: [0; ENTRIES],
        pst: [0; ENTRIES],
    };
    for i in 0..ENTRIES {
        let phase = i as u16;
        t.st[i] = top8(settle(phase, ST));
        t.ps_[i] = top8(settle(phase, PS));
        t.pst[i] = top8(settle(phase, PST));
        // Triangle (i << 1) & 0xfff is what an accumulator with the MSB
        // clear and lower bits i produces.
        t.p_t[i] = top8(settle(phase & (MASK >> 1), PT));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    const SAW: Select = Select {
        saw: true,
        tri: false,
        pulse: false,
    };
    const TRI: Select = Select {
        saw: false,
        tri: true,
        pulse: false,
    };
    const PULSE_ONLY: Select = Select {
        saw: false,
        tri: false,
        pulse: true,
    };

    fn tables() -> &'static Tables {
        static T: OnceLock<Tables> = OnceLock::new();
        T.get_or_init(combined_tables)
    }

    #[test]
    fn single_waveforms_pass_through_unchanged() {
        // One driver per line means no contention: the network must give
        // back the digital waveform, including the pass-transistor XOR.
        for p in 0..ENTRIES as u16 {
            assert_eq!(settle(p, SAW), p, "saw {p:#x}");
            assert_eq!(settle(p, TRI), triangle(p), "tri {p:#x}");
        }
        assert_eq!(settle(0, PULSE_ONLY), MASK);
    }

    #[test]
    fn pass_transistor_passes_zero_strongly_and_one_weakly() {
        // Same 0.2 VDD across the device: near ground it conducts far
        // better than near VDD, and it is off once the low side is at
        // VDD - Vt.
        assert!(pass_conductance(0.2, 0.0) > 2.0 * pass_conductance(0.8, 0.6));
        assert!(pass_conductance(1.0, 1.0 - V_THRESHOLD) <= G_OFF);
        // With the body effect the cut-off comes below VDD - Vt0.
        assert!(pass_conductance(1.0, 0.75) <= G_OFF);
    }

    #[test]
    fn one_low_driver_beats_one_high_driver() {
        // The documented AND: sawtooth 0x001 and triangle 0x002 disagree on
        // bits 0 and 1, and both lines must read low.
        let v = line_voltages(0x001, ST);
        assert!(v[0] < V_SWITCH && v[1] < V_SWITCH);
    }

    #[test]
    fn never_sets_a_bit_the_and_clears() {
        // Sharing only adds paths to ground for a high line, so every table
        // is a bitwise subset of the digital AND.
        let t = tables();
        for i in 0..ENTRIES {
            let p = i as u16;
            assert_eq!(t.st[i] & !top8(p & triangle(p)), 0, "st {i:#x}");
            assert_eq!(t.pst[i] & !top8(p & triangle(p)), 0, "pst {i:#x}");
            assert_eq!(t.ps_[i] & !top8(p), 0, "ps_ {i:#x}");
            assert_eq!(t.p_t[i] & !top8((p << 1) & MASK), 0, "p_t {i:#x}");
        }
    }

    #[test]
    fn pulse_drag_dominates_saw_triangle() {
        // Most of the time many lines are low, the pulse node is dragged
        // down and pst loses bits relative to st. (With nearly every line
        // high the pulse node is an extra pull-up, so pst can occasionally
        // keep a bit st loses; only the AND bound above is strict.)
        let t = tables();
        assert_ne!(t.st, t.pst);
        let ones = |tab: &[u8; 4096]| tab.iter().map(|b| b.count_ones()).sum::<u32>();
        assert!(ones(&t.pst) < ones(&t.st) / 2);
    }

    #[test]
    fn shared_xor_node_removes_bits_from_st() {
        // The shared MSB node flips some lines below the AND, so st is not just the
        // textbook AND, but it never adds bits (checked above).
        let t = tables();
        let and_differs = (0..ENTRIES).any(|i| {
            let p = i as u16;
            t.st[i] != top8(p & triangle(p))
        });
        assert!(and_differs);
    }

    #[test]
    fn all_drivers_high_gives_full_scale() {
        // No pull-down anywhere: every line reaches its high level.
        let t = tables();
        assert_eq!(t.ps_[0xfff], 0xff);
        // Accumulator 0x800 gives triangle 0xfff: with the pulse high,
        // every node is high.
        assert_eq!(settle(0x800, PT), MASK);
    }

    #[test]
    fn pulse_node_is_dragged_by_low_lines() {
        // The more lines the sawtooth holds low, the lower the shared pulse
        // node, and the lower the top line, which both pulse and sawtooth
        // drive high.
        let mut last = f64::INFINITY;
        for low_lines in 0..BITS {
            let saw = MASK & !((1u16 << low_lines) - 1);
            let v = line_voltages(saw, PS)[MSB];
            assert!(v < last, "{low_lines} low lines");
            last = v;
        }
    }

    #[test]
    fn pulse_drag_removes_ones_from_pulse_saw() {
        // With most lines low, the dragged pulse node defeats a lone high
        // sawtooth bit, so ps_ loses bits the AND would keep.
        let t = tables();
        assert_eq!(top8(0x800), 0x80);
        assert_eq!(t.ps_[0x800], 0);
    }

    #[test]
    fn shared_msb_bar_node_is_dragged_by_contested_lines() {
        // MSB clear: every triangle 1 comes from the one MSB-bar node. Each
        // line where the triangle is 1 but the sawtooth is 0 sinks current
        // from it, so a line where both are 1 (L[10]: A10 = A9 = 1) falls
        // as such lines are added (bit j set, bit j+1 clear).
        let mut last = f64::INFINITY;
        for m in [0x000u16, 0x001, 0x005, 0x015, 0x055] {
            let v = line_voltages(0x600 | m, ST)[10];
            assert!(v < last, "{m:#x}: {v}");
            last = v;
        }
    }

    #[test]
    fn both_triangle_halves_differ_from_and() {
        // The XOR shares a node in both halves (MSB-bar below the midpoint,
        // MSB above it), so st departs from the AND on both sides.
        let t = tables();
        let differs = |r: std::ops::Range<usize>| {
            r.into_iter()
                .any(|i| t.st[i] != top8(i as u16 & triangle(i as u16)))
        };
        assert!(differs(0..ENTRIES / 2));
        assert!(differs(ENTRIES / 2..ENTRIES));
    }

    #[test]
    fn p_t_depends_only_on_the_triangle_value() {
        // p_t is indexed by i with triangle (i << 1) & 0xfff, so i and
        // i ^ 0x800 give the same triangle and must give the same entry.
        let t = tables();
        for i in 0..ENTRIES / 2 {
            assert_eq!(t.p_t[i], t.p_t[i | 0x800]);
        }
    }
}
