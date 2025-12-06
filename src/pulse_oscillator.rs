/// Oscillator arguments
#[derive(Clone, Copy)]
pub struct OsciArgs {
    /// Output volume level (`0..=128`)
    pub volume: i16,
    /// How many samples should there be in the output
    pub sample_num: u32,
}

/// 2d point for [`coord`] and [`overtone`] based wave generation.
#[derive(Copy, Clone, Default, Debug)]
pub struct OsciPt {
    /// X coordinate
    pub x: u16,
    /// Y coordinate
    pub y: i16,
}

impl OsciPt {
    /// `[0, 0]` coordinate
    pub const ZERO: Self = Self { x: 0, y: 0 };
}

/// Get the amplitude of sample at `index` for an [Overtone](<https://en.wikipedia.org/wiki/Overtone>) based
/// wave.
///
/// For overtone based wave generation, for each point:
/// - `x` represents the frequency of the waveform
/// - `y` represents the amplitude of the waveform
///
/// Each point represents a new waveform "overlaid" on top of the previous one.
///
/// NOTE: This doc was written by someone who has no idea about music or sound theory.
/// More accurate docs welcome 😛.
#[must_use]
pub fn overtone(args: OsciArgs, points: &[OsciPt], index: u16) -> f64 {
    let overtone: f64 = points
        .iter()
        .map(|pt| {
            let phase = 2.0
                * std::f64::consts::PI
                * (f64::from(pt.x) * f64::from(index) / f64::from(args.sample_num));
            phase.sin() * f64::from(pt.y) / f64::from(pt.x) / 128.
        })
        .sum();
    overtone * f64::from(args.volume) / 128.
}

/// Get the amplitude of sample at `index` for a coordinate based wave.
///
/// For coordinate based wave generation, for each point:
/// - `x` represents time
/// - `y` represents amplitude
///
/// Each point adds a new coordinate.
///
/// The wave is interpolated over `args.sample_num`, with a
/// "horizontal resolution" factor of `hres`.
/// Normally you want to synchronize `hres` with the maximum `x` point your wave has,
/// but it's allowed to fiddle with `hres` to produce different results.
///
/// # Panics
///
/// Panics if the computed horizontal position cannot fit into an `u16`.
/// This can happen if `args.sample_num` is very small, and `index` and `hres` are very large.
#[must_use]
pub fn coord(args: OsciArgs, points: &[OsciPt], index: u16, hres: u16) -> f64 {
    let len = points.len();
    if len == 0 {
        return 0.0;
    }
    let mut i: u16 = (u32::from(hres) * u32::from(index) / args.sample_num).try_into().unwrap();

    let mut c = 0;
    while c < len {
        if points[c].x > i {
            break;
        }
        c += 1;
    }

    let (x1, y1, x2, y2) = if c == len {
        (points[c - 1].x, points[c - 1].y, hres, points[0].y)
    } else if c != 0 {
        (points[c - 1].x, points[c - 1].y, points[c].x, points[c].y)
    } else {
        (points[0].x, points[0].y, points[0].x, points[0].y)
    };

    let w: u16 = x2 - x1;
    i = i.saturating_sub(x1);
    let h: i16 = y2 - y1;

    let work = if i != 0 {
        f64::from(y1) + f64::from(h) * f64::from(i) / f64::from(w)
    } else {
        f64::from(y1)
    };

    work * f64::from(args.volume) / 128. / 128.
}
