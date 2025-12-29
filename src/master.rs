use crate::{
    result::{ProjectReadError, ReadResult},
    timing::{Meas, Tick, Timing, meas_to_tick, tick_to_meas},
};

/// Timing and loop points
pub struct Master {
    /// The timing data of the song
    pub timing: Timing,
    /// Loop points
    pub loop_points: LoopPoints,
    pub(crate) meas_num: Meas,
}

/// Where the song ends and starts repeating from
#[derive(Default)]
pub struct LoopPoints {
    /// The [`Meas`] the song starts playing from when looped.
    pub repeat: Meas,
    /// The last [`Meas`] the song plays before ending or repeating
    pub last: Meas,
}

impl LoopPoints {
    /// Convert [`Tick`]s into [`Meas`] loop points.
    #[must_use]
    pub fn from_ticks(repeat: Tick, last: Tick, timing: Timing) -> Self {
        Self {
            repeat: tick_to_meas(repeat, timing),
            last: tick_to_meas(last, timing),
        }
    }
}

impl Default for Master {
    fn default() -> Self {
        Self {
            timing: Timing::default(),
            loop_points: LoopPoints::default(),
            meas_num: 1,
        }
    }
}

impl Master {
    pub(crate) fn get_last_tick(&self) -> Tick {
        meas_to_tick(self.loop_points.last, self.timing)
    }

    pub(crate) const fn get_play_meas(&self) -> Meas {
        if self.loop_points.last != 0 {
            self.loop_points.last
        } else {
            self.meas_num
        }
    }

    pub(crate) fn adjust_meas_num(&mut self, tick: Tick) {
        self.meas_num = std::cmp::max(self.meas_num, tick_to_meas(tick, self.timing));
        if self.loop_points.repeat >= self.meas_num {
            self.loop_points.repeat = 0;
        }
        if self.loop_points.last > self.meas_num {
            self.loop_points.last = self.meas_num;
        }
    }

    pub(crate) fn read_v5(rd: &mut crate::io::Reader) -> ReadResult<Self> {
        let size = rd.next::<u32>()?;
        if size != 15 {
            return Err(ProjectReadError::FmtUnknown);
        }
        let ticks_per_beat = rd.next::<u16>()?;
        let beats_per_meas = rd.next::<u8>()?;
        let bpm = rd.next::<f32>()?;
        let repeat_tick = rd.next::<u32>()?;
        let last_tick = rd.next::<u32>()?;

        let timing = Timing {
            ticks_per_beat,
            bpm,
            beats_per_meas,
        };

        Ok(Self {
            timing,
            loop_points: LoopPoints::from_ticks(repeat_tick, last_tick, timing),
            meas_num: 1,
        })
    }

    pub(crate) fn write_v5(&self, out: &mut Vec<u8>) {
        let size: u32 = 15;
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&self.timing.ticks_per_beat.to_le_bytes());
        out.extend_from_slice(&self.timing.beats_per_meas.to_le_bytes());
        out.extend_from_slice(&self.timing.bpm.to_le_bytes());
        let clock_repeat: u32 = meas_to_tick(self.loop_points.repeat, self.timing);
        let clock_last: u32 = meas_to_tick(self.loop_points.last, self.timing);
        out.extend_from_slice(&clock_repeat.to_le_bytes());
        out.extend_from_slice(&clock_last.to_le_bytes());
    }
}
