use crate::{EnvelopeSrc, pulse_oscillator::OsciPt};

/// How to generate a wave voice
#[derive(Clone)]
pub struct WaveData {
    /// The points of the wave
    pub points: WaveDataPoints,
    /// The envelope of the wave
    pub envelope: EnvelopeSrc,
    /// Volume to generate the samples with
    pub volume: i16,
    /// Panning
    pub pan: i16,
}

/// Defines the points of the wave
#[derive(Clone)]
pub enum WaveDataPoints {
    /// Wave generated with [`coord`](crate::coord).
    Coord {
        /// The points to generate the wave from.
        points: Vec<OsciPt>,
        /// Resolution of the wave. Typically should be the same as the biggest x coordinate.
        resolution: u16,
    },
    /// Wave generated with [`overtone`](crate::overtone).
    Overtone {
        /// The points to generate the wave from.
        points: Vec<OsciPt>,
    },
}
