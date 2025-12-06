use crate::{
    SampleRate,
    timing::BpMea,
    unit::{GroupIdx, GroupSamples, MAX_CH_LEN},
};

/// What unit should the delay frequency be treated as
#[derive(Debug, PartialEq, Eq)]
pub enum DelayUnit {
    /// Number of beats
    ///
    /// How long a beat is is defined by [`Timing::bpm`](crate::Timing::bpm).
    Beat = 0,
    /// [`Meas`](crate::timing::Meas)
    Meas,
    /// Number of seconds
    Second,
}

/// A delay (reverb) effect
#[derive(Debug)]
pub struct Delay {
    /// What unit the frequency has
    pub unit: DelayUnit,
    /// Index of the group this delay applies to
    pub group: GroupIdx,
    /// How much to apply the delay effect to the group
    pub rate: u8,
    /// What frequency should the reverb effect have.
    pub freq: f32,
    pub(crate) offset: usize,
    pub(crate) bufs: [Vec<i32>; MAX_CH_LEN],
}

impl Default for Delay {
    fn default() -> Self {
        Self {
            unit: DelayUnit::Beat,
            group: GroupIdx(0),
            rate: Default::default(),
            freq: Default::default(),
            offset: Default::default(),
            bufs: Default::default(),
        }
    }
}

enum BufLenCalcError {
    /// The resulting length would be too large (unintended huge allocation)
    TooLarge,
    /// The frequency is zero, it would result in division by zero
    ZeroFreq,
}

// 2^24, size in bytes is ~67 megabytes
const MAX_BUF_LEN: usize = 16_777_216;

impl Delay {
    /// Returns the buffer length for debugging/inspection purposes
    #[must_use]
    pub const fn buf_len(&self) -> usize {
        self.bufs[0].len()
    }
    /// Rebuild the internal buffers used for the delay effect
    pub fn rebuild(&mut self, bp_mea: BpMea, beat_tempo: f32, sps: SampleRate) {
        self.offset = 0;
        match self.calc_buf_len(bp_mea, beat_tempo, sps) {
            Ok(buf_len) => {
                for buf in &mut self.bufs {
                    *buf = vec![0; buf_len];
                }
            }
            Err(BufLenCalcError::TooLarge) => {
                eprintln!("Resulting buffer for delay would be too large.");
            }
            Err(BufLenCalcError::ZeroFreq) => {
                eprintln!("Can't calc buffer size because frequency is zero.");
            }
        }
    }
    /// Calculate the buffer length to use for the delay.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn calc_buf_len(
        &self,
        bp_mea: BpMea,
        beat_tempo: f32,
        sps: SampleRate,
    ) -> Result<usize, BufLenCalcError> {
        if self.freq == 0.0 {
            return Err(BufLenCalcError::ZeroFreq);
        }
        let size = match self.unit {
            DelayUnit::Beat => (f32::from(sps) * 60. / beat_tempo / self.freq) as usize,
            DelayUnit::Meas => {
                (f32::from(sps) * 60. * f32::from(bp_mea) / beat_tempo / self.freq) as usize
            }
            DelayUnit::Second => (f32::from(sps) / self.freq) as usize,
        };
        if size > MAX_BUF_LEN {
            Err(BufLenCalcError::TooLarge)
        } else {
            Ok(size)
        }
    }

    pub(crate) fn tone_supple(&mut self, ch: u8, group_smps: &mut GroupSamples) {
        // Be resilient against offset overflow (like when configuring delay on the fly)
        let Some(buf_sample) = self.bufs[ch as usize].get(self.offset) else {
            eprintln!("buf sample offset overflow");
            self.offset = 0;
            return;
        };
        group_smps[self.group.usize()] += buf_sample * i32::from(self.rate) / 100;
        self.bufs[ch as usize][self.offset] = group_smps[self.group.usize()];
    }

    pub(crate) const fn tone_increment(&mut self) {
        self.offset += 1;
        if self.offset >= self.bufs[0].len() {
            self.offset = 0;
        }
    }
}
