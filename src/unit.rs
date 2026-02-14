use std::{iter::zip, ops::RangeInclusive};

use crate::{
    Key, MooInstructions, NATIVE_SAMPLE_RATE, SampleRate, SampleT, Timing,
    event::{DEFAULT_BASICKEY, DEFAULT_KEY, DEFAULT_TUNING, DEFAULT_VELOCITY, DEFAULT_VOLUME},
    pulse_frequency::PULSE_FREQ,
    util::ArrayLenExt as _,
    voice::{Voice, VoiceFlags, VoiceTone},
};

/// Unit index
///
/// Maximum allowed number of units by PxTone is 50.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct UnitIdx(pub u8);
impl UnitIdx {
    /// Get the index as a usize
    #[must_use]
    pub fn usize(self) -> usize {
        usize::from(self.0)
    }
}

/// Voice index
///
/// Maximum allowed number of voices by PxTone is 100.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct VoiceIdx(pub u8);
impl VoiceIdx {
    /// Get the index as a usize
    #[must_use]
    pub fn usize(self) -> usize {
        usize::from(self.0)
    }
}

pub const MAX_CHANNEL: u8 = 2;
/// Used to make rust-analyzer happy (doesn't like as casts)
///
/// <https://github.com/rust-lang/rust-analyzer/issues/21165>
pub const MAX_CH_LEN: usize = MAX_CHANNEL as usize;

/// Buffer to store a [`Unit`]'s audio samples before applying the pan time effect.
pub type PanTimeBuf = [i32; 64];

#[derive(Clone)]
/// A 🐄 cow that moos a channel of your song, otherwise known as a unit.
///
/// A unit needs a [`Voice`] to be able to moo. Otherwise it's a silent cow.
///
/// A song will set the voice using a [`SetVoice`](crate::EventPayload::SetVoice) event.
///
/// # Role in the rendering process
///
/// During rendering, the output of each unit will be rendered into so-called "sample groups",
/// which can have various effects, like [`Delay`](crate::Delay) and [`Overdrive`](crate::Overdrive) applied to it.
///
/// Finally, the sample groups are mixed together to give the final output of [`Herd::moo`](crate::Herd::moo).
///
#[doc = include_str!("../doc/svg/units-to-final.svg")]
///
/// # Pan-time effect visualization
///
/// The pan-time effect is used to play the left and right channels at different offsets, to give
/// the stereo effect more depth.
///
#[doc = include_str!("../doc/svg/pantime-render.svg")]
pub struct Unit {
    /// The name of the unit
    pub name: String,
    /// The key at which we are mooing now
    pub key_now: Key,
    /// They key at which we start mooing
    pub key_start: Key,
    /// Used in portamento for the target key to slide to
    pub key_margin: Key,
    /// Where we are during portamento slide
    pub porta_pos: SampleT,
    /// Where we need to go during portamento slide
    pub porta_destination: SampleT,
    /// The left and right channels are each multiplied by this
    pub pan_vols: [i16; MAX_CH_LEN],
    /// The offsets used for the pan time effect
    pub pan_time_offs: [PanTimeOff; MAX_CH_LEN],
    /// This is where the unit's samples are written to before applying the pan time effect, and
    /// writing the unit's sample data to the group buffers.
    pub pan_time_bufs: [PanTimeBuf; MAX_CH_LEN],
    /// Determines the output volume of the unit along with [`velocity`](Self::velocity).
    ///
    /// Normally ranges from 0 to 128, but some songs can set it to values otuside of that range.
    pub volume: i16,
    /// How "hard" a note is hit. Serves the same purpose as [`volume`](Self::volume).
    ///
    /// Volume and velocity both play into determining the output volume.
    ///
    /// Normally ranges from 0 to 128, but some songs can set it to values otuside of that range.
    pub velocity: i16,
    /// Sample group this unit belongs to
    pub group: GroupIdx,
    /// Fine tuning of the mooing frequency, where 1.0 is the normal frequency
    pub tuning: f32,
    /// Which voice the unit should be playing
    pub voice_idx: VoiceIdx,
    /// The voice tones for each channel
    pub tones: [VoiceTone; MAX_CH_LEN],
    /// Whether this unit is muted
    pub mute: bool,
}

/// Pan-time offset.
pub type PanTimeOff = u8;

impl Default for Unit {
    fn default() -> Self {
        Self {
            name: String::default(),
            key_now: Default::default(),
            key_start: Default::default(),
            key_margin: Default::default(),
            porta_pos: Default::default(),
            porta_destination: Default::default(),
            pan_vols: Default::default(),
            pan_time_offs: Default::default(),
            pan_time_bufs: [[0; _]; _],
            volume: Default::default(),
            velocity: Default::default(),
            group: GroupIdx::default(),
            tuning: Default::default(),
            tones: [VoiceTone::default(), VoiceTone::default()],
            voice_idx: VoiceIdx(0),
            mute: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
/// A group index.
pub struct GroupIdx(pub u8);

impl GroupIdx {
    /// The maximum possible group index (inclusive)
    #[expect(clippy::cast_possible_truncation)]
    pub const MAX: Self = Self((GroupSamples::LEN - 1) as u8);
    /// Returns the group index as a usize
    #[must_use]
    pub const fn usize(self) -> usize {
        self.0 as usize
    }
}

pub type GroupSamples = [i32; 7];

impl Unit {
    pub(crate) fn tone_init(&mut self) {
        self.group = GroupIdx(0);
        self.velocity = DEFAULT_VELOCITY.cast_signed();
        self.volume = DEFAULT_VOLUME.cast_signed();
        self.tuning = DEFAULT_TUNING;
        self.porta_destination = 0;
        self.porta_pos = 0;

        for i in 0..MAX_CHANNEL {
            self.pan_vols[i as usize] = 64;
            self.pan_time_offs[i as usize] = 0;
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss
    )]
    pub(crate) fn tone_envelope(&mut self, voices: &[Voice]) {
        let Some(voice) = voices.get(self.voice_idx.usize()) else {
            return;
        };

        for (voice_inst, voice_tone) in zip(&voice.insts, &mut self.tones) {
            if voice_tone.life_count > 0 && !voice_inst.env.is_empty() {
                if voice_tone.on_count > 0 {
                    if voice_tone.env_pos < voice_inst.env.len() {
                        voice_tone.env_volume = voice_inst.env[voice_tone.env_pos];
                        voice_tone.env_pos += 1;
                    }
                } else {
                    voice_tone.env_volume = (i32::from(voice_tone.env_start)
                        + (0 - i32::from(voice_tone.env_start)) * voice_tone.env_pos as i32
                            / i32::try_from(voice_inst.env_release).unwrap())
                        as u8;
                    voice_tone.env_pos += 1;
                }
            }
        }
    }

    pub(crate) const fn tone_key_on(&mut self) {
        self.key_now = self.key_start + self.key_margin;
        self.key_start = self.key_now;
        self.key_margin = 0;
    }

    pub(crate) fn tone_zero_lives(&mut self) {
        for ch in 0..MAX_CHANNEL as usize {
            self.tones[ch].life_count = 0;
        }
    }

    pub(crate) const fn tone_key(&mut self, key: Key) {
        self.key_start = self.key_now;
        self.key_margin = key - self.key_start;
        self.porta_pos = 0;
    }

    pub(crate) fn tone_pan_volume(&mut self, vol: u8) {
        self.pan_vols[0] = 64;
        self.pan_vols[1] = 64;
        if vol >= 64 {
            self.pan_vols[0] = 128 - i16::from(vol);
        } else {
            self.pan_vols[1] = i16::from(vol);
        }
    }

    pub(crate) fn tone_pan_time(&mut self, pan_time: PanTime, sps: SampleRate) {
        self.pan_time_offs = pan_time.to_lr_offsets(sps);
    }

    pub(crate) const fn tone_supple(
        &self,
        group_smps: &mut GroupSamples,
        ch: u8,
        time_pan_index: usize,
    ) {
        let idx = (time_pan_index.wrapping_sub(self.pan_time_offs[ch as usize] as usize))
            & (PanTimeBuf::LEN - 1);
        group_smps[self.group.usize()] += self.pan_time_bufs[ch as usize][idx];
    }
    #[expect(clippy::cast_possible_truncation)]
    pub(crate) fn tone_increment_key(&mut self) -> i32 {
        if self.porta_destination != 0 && self.key_margin != 0 {
            if self.porta_pos < self.porta_destination {
                self.porta_pos += 1;
                self.key_now = (f64::from(self.key_start)
                    + f64::from(self.key_margin) * f64::from(self.porta_pos)
                        / f64::from(self.porta_destination)) as i32;
            } else {
                self.key_now = self.key_start + self.key_margin;
                self.key_start = self.key_now;
                self.key_margin = 0;
            }
        } else {
            self.key_now = self.key_start + self.key_margin;
        }
        self.key_now
    }

    pub(crate) fn tone_increment_sample(&mut self, freq: f32, voices: &[Voice]) {
        let Some(voice) = voices.get(self.voice_idx.usize()) else {
            // If for some reason there is no voice, we just don't do anything
            // instead of panicking
            return;
        };

        for ((voice_inst, voice_tone), voice_unit) in
            zip(&voice.insts, &mut self.tones).zip(&voice.units)
        {
            if voice_tone.life_count > 0 {
                voice_tone.life_count -= 1;
            }
            if voice_tone.life_count > 0 {
                voice_tone.on_count -= 1;

                voice_tone.smp_pos += f64::from(voice_tone.offset_freq * self.tuning * freq);

                if voice_tone.smp_pos >= f64::from(voice_inst.num_samples) {
                    if voice_unit.flags.contains(VoiceFlags::WAVE_LOOP) {
                        if voice_tone.smp_pos >= f64::from(voice_inst.num_samples) {
                            voice_tone.smp_pos -= f64::from(voice_inst.num_samples);
                        }
                        if voice_tone.smp_pos >= f64::from(voice_inst.num_samples) {
                            voice_tone.smp_pos = 0.;
                        }
                    } else {
                        voice_tone.life_count = 0;
                    }
                }

                if voice_tone.on_count == 0 && !voice_inst.env.is_empty() {
                    voice_tone.env_start = voice_tone.env_volume;
                    voice_tone.env_pos = 0;
                }
            }
        }
    }

    pub(crate) const fn set_voice(&mut self, idx: VoiceIdx) {
        self.voice_idx = idx;
        self.key_now = DEFAULT_KEY;
        self.key_margin = 0;
        self.key_start = DEFAULT_KEY;
    }

    pub(crate) fn new() -> Self {
        Self {
            name: "<no name>".into(),
            ..Default::default()
        }
    }
    /// Reset the unit's voice to the voice indexed by `voice_idx`
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn reset_voice(&mut self, ins: &MooInstructions, mut voice_idx: VoiceIdx, timing: Timing) {
        if voice_idx.usize() >= ins.voices.len() {
            eprintln!("Error: Voice index out of bounds. Setting to 0.");
            voice_idx = VoiceIdx(0);
        }
        self.set_voice(voice_idx);
        let Some(voice) = &ins.voices.get(voice_idx.usize()) else {
            eprintln!("Error: Song doesn't have any voices");
            return;
        };

        for ((vu, inst), tone) in zip(&voice.units, &voice.insts).zip(&mut self.tones) {
            tone.life_count = 0;
            tone.on_count = 0;
            tone.smp_pos = 0.0;
            tone.env_release_clock = (inst.env_release as f32 / ins.samples_per_tick) as u32;
            tone.offset_freq = if vu.flags.contains(VoiceFlags::BEAT_FIT) {
                (inst.num_samples as f32 * timing.bpm)
                    / (f32::from(NATIVE_SAMPLE_RATE) * 60. * vu.tuning)
            } else {
                #[expect(clippy::cast_possible_wrap)]
                (PULSE_FREQ.get((DEFAULT_BASICKEY as i32).wrapping_sub(vu.basic_key)) * vu.tuning)
            };
        }
    }
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) fn tone_sample(
        &mut self,
        time_pan_index: usize,
        smooth_smp: SampleRate,
        voices: &[Voice],
    ) {
        let Some(voice) = &voices.get(self.voice_idx.usize()) else {
            // If for whatever reason there is no voice, we just don't produce any output
            // instead of panicking
            return;
        };

        for ch in 0..i32::from(MAX_CHANNEL) {
            let mut time_pan_buf: i32 = 0;
            for ((voice_tone, voice_inst), vu) in zip(&self.tones, &voice.insts).zip(&voice.units) {
                // Prevent bytemuck alignment mismatch for empty `smp_w`
                // Should (probably) only happen on dummy read (unimplemented) features.
                if voice_inst.sample_buf.is_empty() {
                    continue;
                }
                let smp_w: &[i16] = bytemuck::cast_slice(&voice_inst.sample_buf);

                let mut work: i32 = 0;

                if voice_tone.life_count > 0 {
                    let pos: i32 = (voice_tone.smp_pos as i32) * 4 + ch * 2;
                    // Theoretically this shouldn't index OOB, but it can happen in weird
                    // configurations, like low sample rate, etc.
                    // We avoid panicking in those cases
                    if let Some(w_sample) = smp_w.get(pos as usize / 2) {
                        work += i32::from(*w_sample);
                    }

                    work = (work * i32::from(self.velocity)) / 128;
                    work = (work * i32::from(self.volume)) / 128;
                    work = work * i32::from(self.pan_vols[ch as usize]) / 64;

                    if !voice_inst.env.is_empty() {
                        work = work * i32::from(voice_tone.env_volume) / 128;
                    }

                    if vu.flags.contains(VoiceFlags::SMOOTH)
                        && voice_tone.life_count < i32::from(smooth_smp)
                    {
                        work = work * voice_tone.life_count / i32::from(smooth_smp);
                    }
                }
                time_pan_buf += work;
            }
            self.pan_time_bufs[ch as usize][time_pan_index] = time_pan_buf;
        }
    }
}

fn calc_pan_time(mut offset: u8, out_sps: SampleRate) -> u8 {
    if offset > 63 {
        offset = 63;
    }
    // If conversion fails due to `out_sps` being signifcantly lower than `NATIVE_SAMPLE_RATE`,
    // we just return 0 as a fallback.
    ((u32::from(offset) * u32::from(NATIVE_SAMPLE_RATE)) / u32::from(out_sps))
        .try_into()
        .unwrap_or(0)
}

/// Inverse of `calc_pan_time`
fn inv_calc_pan_time(val: u8, sps: SampleRate) -> u8 {
    if val == 0 {
        return 0;
    }

    ((u32::from(val) * u32::from(sps)) / u32::from(NATIVE_SAMPLE_RATE)).min(63) as u8
}

/// Sets an effect where the left and right audio channels for the unit are sampled at different
/// offsets.
///
/// Range is within `0..128`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanTime(pub u8);

impl Default for PanTime {
    fn default() -> Self {
        Self(64)
    }
}

impl PanTime {
    /// The valid range of values for pan time
    pub const RANGE: RangeInclusive<u8> = 0..=127;
    /// Calculate the pantime from the raw left and right offsets
    #[must_use]
    pub fn from_lr_offsets(offs: [u8; 2], sps: SampleRate) -> Self {
        match offs {
            [l, 0] if l > 0 => {
                let off = inv_calc_pan_time(l, sps);
                Self(64 + off)
            }
            [0, r] if r > 0 => {
                let off = inv_calc_pan_time(r, sps);
                Self(64 - off)
            }
            _ => Self(64),
        }
    }
    /// Convert the pan time to left and right offsets
    #[must_use]
    pub fn to_lr_offsets(self, sps: SampleRate) -> [u8; 2] {
        if self.0 >= 64 {
            [calc_pan_time(self.0 - 64, sps), 0]
        } else {
            [0, calc_pan_time(64 - self.0, sps)]
        }
    }
}
