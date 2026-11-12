<p align="center">
  <img src="assets/logo.svg" alt="mos6581: a cheerful 28-pin chip" width="480">
</p>

# mos6581

A cycle-stepped MOS 6581 SID chip emulator in Rust. The chip is stepped one Φ2
cycle at a time and has no runtime dependencies. It is only
the chip: no CPU, no memory map, no player. See [`sid-play`](https://github.com/aovestdipaperino/sid-play) for a PSID player built
on it.

Licence: MIT OR Apache-2.0, with no GPL code or data and no optional GPL build.

## Fidelity

The chip models are clean-room work. The combined waveforms and the filter
cutoff curve are electrical models of the 6581 written from public sources; the
envelope periods, voice and mixer DC, resonance, noise taps and output-stage
constants come from the datasheet and public documents, or are plain choices.
[`docs/provenance.md`](https://github.com/aovestdipaperino/mos6581/blob/main/docs/provenance.md)
lists every source, how close each model came to the real chip, and the known
fidelity gaps.
