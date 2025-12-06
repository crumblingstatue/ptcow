use crate::unit::{GroupIdx, GroupSamples};

/// Overdrive effect that amplifies and cuts the samples of a sample group
///
/// The samples are signed 32 bit samples, but the effective range is signed 16 bit
#[must_use]
pub struct Overdrive {
    /// Whether this effect is on
    pub on: bool,
    /// Which sample group this effect operates on
    pub group: GroupIdx,
    /// Cut this percentage of the amplitude
    ///
    /// The larger the percentage, the more is cut.
    /// 0 doesn't cut anything, and 100 cuts everything.
    pub cut_percent: f32,
    /// Multiply (amplify) the samples by this much
    pub amp_mul: f32,
    pub(crate) cut_16bit_top: i32,
}

impl Default for Overdrive {
    fn default() -> Self {
        Self {
            on: false,
            group: GroupIdx(0),
            cut_percent: 0.0,
            amp_mul: 0.0,
            cut_16bit_top: 0,
        }
    }
}

impl Overdrive {
    /// The cut percentage must be within this range
    pub const CUT_VALID_RANGE: std::ops::RangeInclusive<f32> = 50.0..=99.9;
    /// The amplitude multiplication factor must be within this range
    pub const AMP_VALID_RANGE: std::ops::RangeInclusive<f32> = 0.1..=8.0;
    #[expect(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub(crate) fn tone_supple(&self, group_smps: &mut GroupSamples) {
        if !self.on {
            return;
        }
        let mut work: i32 = group_smps[self.group.usize()];
        if work > self.cut_16bit_top {
            work = self.cut_16bit_top;
        } else if work < -self.cut_16bit_top {
            work = -self.cut_16bit_top;
        }
        group_smps[self.group.usize()] = (work as f32 * self.amp_mul) as i32;
    }
    /// Rebuild the internal data used to produce this effect
    #[expect(clippy::cast_possible_truncation)]
    pub fn rebuild(&mut self) {
        self.cut_16bit_top = (32767.0 * (100.0 - self.cut_percent) / 100.0) as i32;
    }
}
