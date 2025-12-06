🐄🐄🐄🐄🐄🐄🐄🐄🐄🐄\
🐮 ptcow 🐮\
🐄🐄🐄🐄🐄🐄🐄🐄🐄🐄
=======================

Library for editing and playback of PxTone (.ptcop) music.

Based on the PxTone C++ source code available [here](<https://pxtone.org/developer/>).

## ✅Goals / ❌Non-goals

- ✅ Support V5 (and newer) versions of PxTone.
- ❌ No support for V4 and earlier. Maybe read support in the future, but no export support planned.

- ✅ Rendering that sounds faithful to the original PxTone rendering
- ❌ No sample-by-sample accuracy. There can be minor differences as long as it sounds (almost) indistinguishable.

## Getting Started

To get started, load a `.ptcop` or `.pttune` file into a `Vec<u8>`, and call [`read_song`] on it.
You can also check out `crates/ptmoo` for a command line player that writes samples to stdout.
