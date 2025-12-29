# Changelog

## 0.3.0 - 2025.12.29

### ptcow

- (**Breaking**) Turn `MooPlan`'s `meas_end` and `meas_repeat` fields into `Option`s.
- (**Breaking**) Turn `LoopPoints::last` into `Option<NonZeroMeas>`
- Export `LoopPoints` type as public
- Add `LoopPoints::from_ticks`

## 0.2.1 - 2025.12.26

### ptcow

- Fix compile error when compiling without Ogg/Vorbis support
- Make lack of big endian support explicit
- When recalculating voice envelope, properly remove output envelope if source doesn't exist
- Minor documentation improvements

### ptmoo

- Use crossterm instead of raw ANSI sequences
- Fix flickering and visualization artifacts in terminal output

## 0.2.0 - 2025.12.06

### ptcow

- Add more detailed info to data read errors
- Lower MSRV to 1.88
- Implement Ogg/Vorbis voices properly instead of reading/writing them as PCM voices.
- Fix incorrect sample count for stereo Ogg/Vorbis voices

### ptmoo
- Abort playback if stdout is a terminal
- Improve error reporting

## 0.1.0 - 2025.12.06

Initial release.