//! Timing related definitions and utilities
use {crate::SampleRate, std::num::NonZeroU32};

/// Clock ticks.
///
/// Ticks are the time granularity that [`Event`](crate::Event)s happen at.
pub type Tick = u32;
/// A smaller clock [`Tick`] type
pub type Tick16 = u16;

/// Measure, also known as a bar. It groups beats together.
///
/// [`Timing::beats_per_meas`] defines how many beats are in a `Meas`.
pub type Meas = u32;
/// A non-zero [`Meas`].
pub type NonZeroMeas = NonZeroU32;

/// The smallest unit of time we deal with; an audio sample.
pub type SampleT = u32;

/// How many samples constitute a tick.
pub type SamplesPerTick = f32;

const DEFAULT_BEATS_PER_MEAS: u8 = 4;
const DEFAULT_BPM: f32 = 120.;
const DEFAULT_TICKS_PER_BEAT: Tick16 = 480;

/// Timing related information
#[derive(Clone, Copy)]
pub struct Timing {
    /// How many clock ticks happen during a beat
    ///
    /// For example if bpm is 1 and ticks per beat is 60,
    /// then 1 tick happens per second (60 ticks per minute).
    ///
    /// The higher this value, the more ticks happen per beat, the faster the song plays.
    pub ticks_per_beat: Tick16,
    /// Beats per minute
    ///
    /// The higher, the faster the song plays
    pub bpm: f32,
    /// How many beats are in a [`Meas`]
    pub beats_per_meas: BpMea,
}

/// Beats per [`Meas`]
pub type BpMea = u8;

impl Default for Timing {
    fn default() -> Self {
        Self {
            ticks_per_beat: DEFAULT_TICKS_PER_BEAT,
            bpm: DEFAULT_BPM,
            beats_per_meas: DEFAULT_BEATS_PER_MEAS,
        }
    }
}

/// Converts [`Tick`]s to [`Meas`]
#[must_use]
pub fn tick_to_meas(tick: Tick, timing: Timing) -> Meas {
    tick.div_ceil(u32::from(timing.ticks_per_beat))
        .div_ceil(u32::from(timing.beats_per_meas))
}

/// Converts [`Meas`] to [`Tick`]s.
#[must_use]
pub fn meas_to_tick(meas: Meas, timing: Timing) -> Tick {
    meas * Tick::from(timing.ticks_per_beat) * Tick::from(timing.beats_per_meas)
}

/// Calculates how many samples make up a tick.
#[must_use]
pub fn samples_per_tick(out_sample_rate: SampleRate, timing: Timing) -> SamplesPerTick {
    60.0 * f32::from(out_sample_rate) / (timing.bpm * f32::from(timing.ticks_per_beat))
}

/// Converts [`Tick`]s to a number of [samples](SampleT).
#[must_use]
pub fn tick_to_sample(tick: Tick, samples_per_tick: SamplesPerTick) -> SampleT {
    // We do a number of lossy casts here, but this is the behavior of original PxTone as well.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    ((tick as f32 * samples_per_tick) as SampleT)
}

/// Converts [`Meas`] to a number of [samples](SampleT).
#[must_use]
pub fn meas_to_sample(meas: Meas, samples_per_tick: SamplesPerTick, timing: Timing) -> SampleT {
    // Note: Yes, this does need to use f64 to remain sample accurate with original PxTone playback
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    ((f64::from(meas)
        * f64::from(timing.beats_per_meas)
        * f64::from(timing.ticks_per_beat)
        * f64::from(samples_per_tick)) as SampleT)
}
