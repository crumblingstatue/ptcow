use crate::pulse_oscillator::OsciPt;
/// What kind of wave to generate
#[derive(Clone)]
pub enum WaveData {
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
