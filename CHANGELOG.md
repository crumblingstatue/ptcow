# Changelog

## 0.5.0 - 2026.03.07

- Switch to symphonia for Ogg/Vorbis decoding (it has web support)
- Remove a lot of panics by doing fallback behavior instead of panicking.
  The new policy is that panicking is a bug. This is necessary for proper web
  support, where you can't catch panics, your app just crashes.
- Expose `Herd::moo_end`
- Add sort method to `EveList`. Its invariant is to always be sorted by tick.
- Impl `Deref` and `DerefMut` for `EveList`
- Truncate too long unit and voice names when saving, rather than panicking
- Use `ArrayVec` in places where there is a fixed limit for the number of items,
  like units and voices.
- Implement `Clone` for `Voice`
- Add `PanTime` type
- Derive `Debug` and `Hash` for `UnitIdx`
- Various fixes to `NoiseBuilder` behavior
- Add `NoiseData::from_ptnoise/to_ptnoise` and `Voice::from_ptvoice/to_ptvoice`
- Add public `Voice::recalculate` method for recalculating voice sample data
- Derive `Default` for `VoiceIdx`
- Add `Units` type to abstract the container for units. It can be indexed with `UnitIdx`
- Add `Voices` type to do the same for voices. Indexed by `VoiceIdx`.
- Bundle together voice units and voice instances into a `VoiceSlot` struct.
- Move envelope source from `VoiceUnit` to `WaveData`, as it's only used by the latter.
- Get rid of serialization flags for `NoiseData`,
  basing whether to serialize on volume and frequency not being 0.
- Better error message for `ProjectWriteError::CoordWavePointOutOfRange`

A bunch of these are breaking changes, but I can't bother sorting them individually.

## 0.4.0 - 2026.01.17

### ptcow

- (**Breaking**) Add `VoiceIdx` type for voice indices
- (**Breaking**) Rename `Master::get_play_meas` to `end_meas` and make it public
- Add `Song::recalculate_length`
- Use bitflags for `NoiseDesignUnit`'s io flags, and make it public
- Publicly export `DEFAULT_KEY`
- Add some useful derives
- Make sample type on `moo` generic
- Make `Unit::reset_voice` public

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