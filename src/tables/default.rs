//! Built-in data: the clean-room cutoff curve and combined waveforms the chip
//! uses, plus the earlier textbook baselines kept for reference. Our own
//! derivations, MIT OR Apache-2.0.

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
/// waveforms. A reference baseline only: the chip uses `clean_waves`.
pub static WAVE_AND: WaveTables = and_tables();

/// Cutoff points for the built-in curve: linear over the MOS 6581 datasheet's
/// nominal filter range, "approximately 30 Hz - 12 kHz" with 2200 pF filter
/// capacitors. The C64 fits other values and real chips vary. A reference
/// baseline only: the chip uses `clean_cutoff_table`.
pub const CUTOFF_POINTS: &[(u32, f64)] = &[(0, 30.0), (2047, 12_000.0)];

/// The built-in 6581 cutoff curve: the clean-room VCR model (see
/// docs/provenance.md), no longer the linear curve.
pub fn clean_cutoff_table() -> &'static super::CutoffTable {
    static T: std::sync::OnceLock<super::CutoffTable> = std::sync::OnceLock::new();
    T.get_or_init(|| super::fc::build_from(super::clean_cutoff::cutoff_hz_6581))
}

/// The built-in 6581 combined waveforms: the clean-room combined-waveform model
/// (see `docs/provenance.md`), no longer the AND tables.
///
/// The tables are precomputed in `clean_waves_data.rs`; solving the model on
/// first use cost about 0.6 s in release and 13 s in debug per process.
pub fn clean_waves() -> &'static WaveTables {
    &super::clean_waves_data::CLEAN_WAVES
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
    fn the_clean_seams_carry_the_clean_models() {
        let c = clean_cutoff_table();
        assert!((c.f0[1000] - super::super::clean_cutoff::cutoff_hz_6581(1000)).abs() < 1e-12);
        assert_ne!(clean_waves().pst, clean_waves().st);
    }

    #[test]
    fn the_default_build_routes_through_the_seams() {
        let w = crate::tables::for_model(crate::Model::Pal);
        assert!(std::ptr::eq(w, clean_waves()));
        let c = crate::tables::cutoff_table(crate::Model::Pal);
        assert!(std::ptr::eq(c, clean_cutoff_table()));
    }

    /// The precomputed constant must equal what the model produces.
    ///
    /// Solving the model takes about 13 s in a debug build, so this runs only
    /// in release builds (`cargo test --release`).
    #[test]
    #[cfg_attr(debug_assertions, ignore)]
    fn the_precomputed_waves_match_the_model() {
        let c = crate::tables::clean_waves::combined_tables();
        let w = clean_waves();
        assert!(c.st == w.st, "st drifted from the model");
        assert!(c.p_t == w.p_t, "p_t drifted from the model");
        assert!(c.ps_ == w.ps_, "ps_ drifted from the model");
        assert!(c.pst == w.pst, "pst drifted from the model");
    }

    /// Rewrites `src/tables/clean_waves_data.rs` from the model. Run with
    /// `cargo test --release regenerate_clean_waves_data -- --ignored`.
    #[test]
    #[ignore = "regenerates src/tables/clean_waves_data.rs"]
    fn regenerate_clean_waves_data() {
        use std::fmt::Write;
        let c = crate::tables::clean_waves::combined_tables();
        let mut out = String::from(
            "//! Generated from the clean-room model in `clean_waves.rs`; do not edit.\n\
             //! Regenerate with `cargo test --release regenerate_clean_waves_data -- --ignored`.\n\
             //! The test `the_precomputed_waves_match_the_model` keeps it in step.\n\n\
             use super::WaveTables;\n\n\
             #[rustfmt::skip]\n\
             pub(crate) static CLEAN_WAVES: WaveTables = WaveTables {\n",
        );
        for (name, t) in [
            ("st", &c.st),
            ("p_t", &c.p_t),
            ("ps_", &c.ps_),
            ("pst", &c.pst),
        ] {
            writeln!(out, "    {name}: [").unwrap();
            for row in t.chunks(16) {
                let v: Vec<String> = row.iter().map(|b| format!("{b:#04x}")).collect();
                writeln!(out, "        {},", v.join(", ")).unwrap();
            }
            out.push_str("    ],\n");
        }
        out.push_str("};\n");
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/tables/clean_waves_data.rs"
        );
        std::fs::write(path, out).unwrap();
    }
}
