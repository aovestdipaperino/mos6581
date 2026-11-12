//! Cutoff register -> centre frequency, as a 2048-entry table.

pub struct CutoffTable {
    pub f0: [f64; 2048],
}

/// A table from a function of the register value, for curves given as a model
/// rather than as points.
pub(crate) fn build_from(f: impl Fn(u16) -> f64) -> CutoffTable {
    let mut f0 = [0.0; 2048];
    for (i, slot) in f0.iter_mut().enumerate() {
        *slot = f(i as u16);
    }
    CutoffTable { f0 }
}

/// The 6581 is the modelled chip, so both clock variants share its curve, the
/// clean-room one from `default::clean_cutoff_table`.
pub fn cutoff_table(model: crate::Model) -> &'static CutoffTable {
    match model {
        crate::Model::Pal | crate::Model::Ntsc => super::default::clean_cutoff_table(),
    }
}
