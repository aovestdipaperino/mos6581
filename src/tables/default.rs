//! Built-in data used when the `gpl-data` feature is off. Our own derivations,
//! MIT OR Apache-2.0.

use super::WaveTables;

/// Triangle output for a phase, as the oscillator produces it with no ring
/// modulation and the accumulator's low 12 bits zero.
const fn triangle(phase: u16) -> u16 {
    if phase & 0x800 != 0 {
        ((!phase << 1) | 1) & 0x0fff
    } else {
        (phase << 1) & 0x0fff
    }
}

const fn and_tables() -> WaveTables {
    let mut t = WaveTables {
        st: [0; 4096],
        p_t: [0; 4096],
        ps_: [0; 4096],
        pst: [0; 4096],
    };
    let mut i = 0;
    while i < 4096 {
        let phase = i as u16;
        let saw_tri = phase & triangle(phase);
        t.st[i] = (saw_tri >> 4) as u8;
        // p_t is indexed by triangle >> 1; with pulse high it passes the triangle.
        t.p_t[i] = (((phase << 1) & 0x0fff) >> 4) as u8;
        t.ps_[i] = (phase >> 4) as u8;
        t.pst[i] = (saw_tri >> 4) as u8;
        i += 1;
    }
    t
}

/// Combined waveforms approximated as the bitwise AND of the pure waveforms:
/// the textbook model. Real silicon is quieter and less regular on combined
/// waveforms; the `gpl-data` feature swaps in measured tables.
pub static WAVE_AND: WaveTables = and_tables();

/// Cutoff points for the built-in curve: linear over the MOS 6581 datasheet's
/// nominal filter range, "approximately 30 Hz - 12 kHz" with 2200 pF filter
/// capacitors. The C64 fits other values and real chips vary; the `gpl-data`
/// feature swaps in measured curves.
pub const CUTOFF_POINTS: &[(u32, f64)] = &[(0, 30.0), (2047, 12_000.0)];

/// The built-in 6581 cutoff curve, compiled in every build so it can be compared
/// with the measured one. Today: the linear datasheet curve.
pub fn clean_cutoff_table() -> &'static super::CutoffTable {
    static T: std::sync::OnceLock<super::CutoffTable> = std::sync::OnceLock::new();
    T.get_or_init(|| super::fc::build(CUTOFF_POINTS))
}

/// The built-in 6581 combined waveforms, compiled in every build so they can be
/// compared with the measured ones. Today: the AND tables.
pub fn clean_waves() -> &'static WaveTables {
    &WAVE_AND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saw_triangle_is_the_and_of_the_two() {
        // Phase 0x400: saw 0x400, triangle 0x800, AND 0.
        assert_eq!(WAVE_AND.st[0x400], 0);
        // Phase 0x7ff: saw 0x7ff, triangle 0xffe, AND 0x7fe, >> 4 = 0x7f.
        assert_eq!(WAVE_AND.st[0x7ff], 0x7f);
    }

    #[test]
    fn pulse_alone_passes_the_other_waveform() {
        assert_eq!(WAVE_AND.ps_[0xabc], 0xab);
        assert_eq!(WAVE_AND.p_t[0x400], 0x80);
    }

    #[test]
    fn three_way_matches_saw_triangle() {
        assert_eq!(WAVE_AND.pst, WAVE_AND.st);
    }

    #[test]
    fn the_clean_seams_carry_todays_defaults() {
        // Until a clean-room model lands, the seams return today's built-ins.
        let c = clean_cutoff_table();
        assert!((c.f0[0] - 30.0).abs() < 1e-9);
        assert!((c.f0[2047] - 12_000.0).abs() < 1e-9);
        assert!(std::ptr::eq(clean_waves(), &WAVE_AND));
    }

    #[cfg(not(feature = "gpl-data"))]
    #[test]
    fn the_default_build_routes_through_the_seams() {
        let w = crate::tables::for_model(crate::Model::Pal);
        assert!(std::ptr::eq(w, clean_waves()));
        let c = crate::tables::cutoff_table(crate::Model::Pal);
        assert!(std::ptr::eq(c, clean_cutoff_table()));
    }
}
