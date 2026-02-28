use crate::{EnvelopeSrc, pulse_oscillator::OsciPt};

#[derive(Clone)]
pub struct WaveData {
    pub inner: WaveDataInner,
    pub envelope: EnvelopeSrc,
}

/// What kind of wave to generate
#[derive(Clone)]
pub enum WaveDataInner {
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
