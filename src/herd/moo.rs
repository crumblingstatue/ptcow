use {
    crate::{
        Meas, NATIVE_SAMPLE_RATE, SampleRate, SampleT,
        event::{EveList, Event, EventPayload},
        herd::{Herd, MooInstructions, Song},
        master::Master,
        pulse_frequency::PULSE_FREQ,
        timing::{self, Tick, meas_to_sample},
        unit::{MAX_CHANNEL, PanTimeBuf, UnitIdx},
        util::ArrayLenExt as _,
    },
    std::{iter::zip, ops::ControlFlow},
};

/// Get the current [`Tick`] the playback is at.
#[expect(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
#[must_use]
pub fn current_tick(herd: &Herd, ins: &MooInstructions) -> Tick {
    (herd.smp_count as f32 / ins.samples_per_tick) as u32
}

#[expect(clippy::cast_possible_truncation)]
pub(super) fn next_sample(
    herd: &mut Herd,
    ins: &MooInstructions,
    events: &EveList,
    master: &Master,
    dst_sps: SampleRate,
    out: &mut [i16; 2],
    advance: bool,
) -> bool {
    for unit in &mut herd.units {
        unit.tone_envelope(&ins.voices);
    }

    if advance {
        let clock = current_tick(herd, ins);

        while herd.evt_idx < events.eves.len() && (events.eves[herd.evt_idx]).tick <= clock {
            if do_next_event(herd, ins, events, master, clock, dst_sps).is_break() {
                break;
            }
        }
    }

    for unit in &mut herd.units {
        unit.tone_sample(herd.time_pan_index, herd.smp_smooth, &ins.voices);
    }

    for ch in 0..MAX_CHANNEL {
        let mut group_smps = [0; _];
        for unit in &mut herd.units {
            if !unit.mute {
                unit.tone_supple(&mut group_smps, ch, herd.time_pan_index);
            }
        }
        for ovr in &mut herd.overdrives {
            ovr.tone_supple(&mut group_smps);
        }
        for delay in &mut herd.delays {
            delay.tone_supple(ch, &mut group_smps);
        }

        let mut out_samp: i32 = 0;

        for group_smp in group_smps {
            out_samp += group_smp;
        }

        out[ch as usize] = out_samp.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
    }
    if advance {
        herd.smp_count += 1;
    }
    herd.time_pan_index = (herd.time_pan_index + 1) & (PanTimeBuf::LEN - 1);

    for unit in &mut herd.units {
        #[expect(clippy::cast_sign_loss)]
        let key_now = unit.tone_increment_key() as usize;
        unit.tone_increment_sample(PULSE_FREQ.get2(key_now) * herd.smp_stride, &ins.voices);
    }

    for delay in &mut herd.delays {
        delay.tone_increment();
    }

    if herd.smp_count >= herd.smp_end {
        if !herd.loop_ {
            return false;
        }
        herd.smp_count = herd.smp_repeat;
        herd.evt_idx = 0;
        herd.tune_cow_voices(ins, master.timing);
    }
    true
}

fn do_next_event(
    herd: &mut Herd,
    ins: &MooInstructions,
    events: &EveList,
    master: &Master,
    clock: Tick,
    dst_sps: SampleRate,
) -> ControlFlow<()> {
    let evt = &events.eves[herd.evt_idx];
    do_event(herd, ins, events, master, clock, dst_sps, evt)?;
    herd.evt_idx += 1;
    ControlFlow::Continue(())
}

/// Do a single event
pub fn do_event(
    herd: &mut Herd,
    ins: &MooInstructions,
    events: &EveList,
    master: &Master,
    clock: u32,
    dst_sps: u16,
    evt: &Event,
) -> ControlFlow<()> {
    let u = evt.unit;
    let Some(unit) = herd.units.get_mut(u.usize()) else {
        return ControlFlow::Break(());
    };

    match evt.payload {
        EventPayload::On { duration } => {
            do_on_event(herd, ins, events, clock, duration, evt.unit, evt.tick);
        }
        EventPayload::Key(key) => unit.tone_key(key),
        EventPayload::PanVol(vol) => unit.tone_pan_volume(vol),
        EventPayload::PanTime(pan) => unit.tone_pan_time(pan, dst_sps),
        EventPayload::Velocity(vel) => unit.velocity = vel,
        EventPayload::Volume(vol) => unit.volume = vol,
        EventPayload::Portament { duration } => {
            unit.porta_destination = timing::tick_to_sample(duration, ins.samples_per_tick);
        }
        EventPayload::BeatClock
        | EventPayload::BeatTempo
        | EventPayload::BeatNum
        | EventPayload::Repeat
        | EventPayload::Last
        | EventPayload::PtcowDebug(_) => {}
        EventPayload::SetVoice(num) => unit.reset_voice(ins, num as usize, master.timing),
        EventPayload::SetGroup(num) => unit.group = num,
        EventPayload::Tuning(tuning) => unit.tuning = tuning,
        EventPayload::Null => return ControlFlow::Break(()),
    }
    ControlFlow::Continue(())
}

#[expect(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn do_on_event(
    herd: &mut Herd,
    ins: &MooInstructions,
    events: &EveList,
    clock: Tick,
    duration: u32,
    u: UnitIdx,
    evt_tick: Tick,
) {
    let Some(unit) = herd.units.get_mut(u.usize()) else {
        return;
    };
    // We need a signed clock here for various calculations that can go below zero
    let clock: i32 = clock.try_into().unwrap();
    // Same for duration
    let duration: i32 = duration.try_into().unwrap();
    let on_count: i32 = ((i32::try_from(evt_tick).unwrap() + duration.saturating_sub(clock)) as f32
        * ins.samples_per_tick) as i32;
    if on_count <= 0 {
        unit.tone_zero_lives();
        return;
    }

    unit.tone_key_on();
    let Some(voice) = ins.voices.get(unit.voice_idx) else {
        eprintln!("Invalid voice idx");
        return;
    };
    for (inst, tone) in zip(&voice.insts, &mut unit.tones) {
        if inst.env_release != 0 {
            let max_life_count1: i32 =
                ((duration - (clock - i32::try_from(evt_tick).unwrap())) as f32)
                    .mul_add(ins.samples_per_tick, inst.env_release as f32) as i32;
            let c = i32::try_from(evt_tick).unwrap()
                + duration
                + i32::try_from(tone.env_release_clock).unwrap();
            let mut next: Option<&Event> = None;
            for i in herd.evt_idx + 1..events.eves.len() {
                let eve = &events.eves[i];
                if i32::try_from(eve.tick).unwrap() > c {
                    break;
                }
                if eve.unit == u && matches!((eve).payload, EventPayload::On { .. }) {
                    next = Some(eve);
                    break;
                }
            }
            let max_life_count2 = match next {
                Some(next) => {
                    ((i32::try_from(next.tick).unwrap() - clock) as f32 * ins.samples_per_tick)
                        as i32
                }
                None => herd.smp_end.cast_signed() - (clock as f32 * ins.samples_per_tick) as i32,
            };
            if max_life_count1 < max_life_count2 {
                tone.life_count = max_life_count1;
            } else {
                tone.life_count = max_life_count2;
            }
        } else {
            tone.life_count = ((duration.saturating_sub(clock - i32::try_from(evt_tick).unwrap()))
                as f32
                * ins.samples_per_tick) as i32;
        }

        if tone.life_count > 0 {
            tone.on_count = on_count;
            tone.smp_pos = 0.;
            tone.env_pos = 0;
            if inst.env.is_empty() {
                tone.env_volume = 128;
                tone.env_start = 128;
            } else {
                tone.env_volume = 0;
                tone.env_start = 0;
            }
        }
    }
}

fn get_total_sample(master: &Master, out_sample_rate: SampleRate) -> u32 {
    calc_sample_num(
        master.meas_num,
        master.timing.beats_per_meas.into(),
        out_sample_rate,
        master.timing.bpm,
    )
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn calc_sample_num(meas_num: u32, beat_num: u32, sps: SampleRate, beat_tempo: f32) -> u32 {
    if beat_tempo == 0. {
        return 0;
    }
    let total_beat_num: u32 = meas_num * beat_num;
    (f64::from(sps) * 60. * f64::from(total_beat_num) / f64::from(beat_tempo)) as u32
}

/// Prepare to [`moo`](Herd::moo).
///
/// # Panics
///
/// - If `ins.out_sample_rate` is 0
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub fn moo_prepare(ins: &mut MooInstructions, herd: &mut Herd, song: &Song, plan: &MooPlan) {
    assert_ne!(ins.out_sample_rate, 0);

    let meas_end = plan.meas_end.unwrap_or_else(|| song.master.end_meas());
    let meas_repeat = plan.meas_repeat.unwrap_or(song.master.loop_points.repeat);

    herd.loop_ = plan.loop_;

    ins.samples_per_tick = timing::samples_per_tick(ins.out_sample_rate, song.master.timing);
    herd.smp_stride = f32::from(NATIVE_SAMPLE_RATE) / f32::from(ins.out_sample_rate);

    herd.time_pan_index = 0;

    herd.smp_end = meas_to_sample(meas_end, ins.samples_per_tick, song.master.timing);
    herd.smp_repeat = meas_to_sample(meas_repeat, ins.samples_per_tick, song.master.timing);

    herd.smp_start = match plan.start_pos {
        StartPosPlan::Meas(val) => meas_to_sample(val, ins.samples_per_tick, song.master.timing),
        StartPosPlan::Sample(val) => val,
        StartPosPlan::F32(val) => {
            (get_total_sample(&song.master, ins.out_sample_rate) as f32 * val) as u32
        }
    };

    herd.smp_count = herd.smp_start;
    herd.smp_smooth = ins.out_sample_rate / 250;

    herd.evt_idx = 0;
    herd.tune_cow_voices(ins, song.master.timing);
}

impl Herd {
    /// Moo the song into a stereo signed 16 bit little endian PCM buffer.
    ///
    /// If `advance` is true, the playback proceeds to the next event.
    /// Setting it to false can be useful for pausing playback, while still allowing
    /// the [`Unit`](crate::Unit)s to play audio.
    pub fn moo(
        &mut self,
        ins: &MooInstructions,
        song: &Song,
        buf: &mut [i16],
        advance: bool,
    ) -> bool {
        if self.end {
            return false;
        }

        for out_samp in buf.as_chunks_mut().0 {
            if !next_sample(
                self,
                ins,
                &song.events,
                &song.master,
                ins.out_sample_rate,
                out_samp,
                advance,
            ) {
                self.end = true;
                break;
            }
        }

        true
    }
}

/// Plan for the cows on how to moo the song
#[derive(Copy, Clone)]
pub struct MooPlan {
    /// Start position
    pub start_pos: StartPosPlan,
    /// End position
    pub meas_end: Option<Meas>,
    /// Repeat position
    pub meas_repeat: Option<Meas>,
    /// Whether to loop the song
    pub loop_: bool,
}

/// Start position that can be given in different units
#[derive(Copy, Clone)]
pub enum StartPosPlan {
    /// Start position as [`Meas`]
    Meas(Meas),
    /// Start position as [`SampleT`]
    Sample(SampleT),
    /// Start position as [`f32`]
    F32(f32),
}
