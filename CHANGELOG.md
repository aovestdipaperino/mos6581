# Changelog

## 0.2.0 - 2026-11-12

Breaking:
- Removed the `gpl-data` feature and the optional `sid-gpl-data` dependency. The crate is MIT OR Apache-2.0 with no optional GPL path.
- Removed `tables::cutoff_table_8580`, `tables::WAVE6581` and `tables::WAVE8580`.

Changed:
- New clean-room 6581 filter cutoff curve and combined waveforms replace the linear datasheet curve and the AND tables. The previous built-ins stay in `tables::default` (`CUTOFF_POINTS`, `WAVE_AND`) as reference baselines. How close the new models come to the measured chip is recorded in `docs/provenance.md`.
- The combined-waveform tables are precomputed (`src/tables/clean_waves_data.rs`, regenerated from the model by an ignored test) instead of solved on first use, removing about 0.6 s (release) or 13 s (debug) of startup per process.

## 0.1.1 / 0.1.0

First releases: a cycle-stepped MOS 6581 emulator, GPL-free by default (datasheet-derived constants, AND-table combined waveforms, linear cutoff curve), with an off-by-default `gpl-data` feature that swapped in measured reSID data from `sid-gpl-data`.
