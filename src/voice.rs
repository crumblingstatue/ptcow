mod io;

use std::iter::zip;

use arrayvec::ArrayVec;

use crate::{
    Key, NATIVE_SAMPLE_RATE, SampleRate,
    event::DEFAULT_BASICKEY,
    noise_builder::{NoiseTable, noise_to_pcm},
    point::EnvPt,
    pulse_oscillator::{OsciArgs, coord, overtone},
    voice_data::{noise::NoiseData, pcm::PcmData, wave::WaveData},
};

#[derive(Clone)]
#[expect(clippy::large_enum_variant)]
/// The data used for the voice waveform
pub enum VoiceData {
    /// Noise generation
    Noise(NoiseData),
    /// Sampled instrument
    Pcm(PcmData),
    /// Wave instrument
    Wave(WaveData),
}

/// Contains the precomputed sample and envelope data for a voice
#[derive(Default, Clone)]
pub struct VoiceInstance {
    /// Number of samples contained in the sample buffer
    ///
    /// This is not the same as the length of the sample buffer, because the sample buffer
    /// contains raw bytes, but a sample is not necessarily a single byte.
    pub num_samples: u32,
    /// Contains the bytes of the samples of the voice used for rendering
    pub sample_buf: Vec<u8>,
    /// Prepared envelope generated from [`VoiceUnit::envelope`].
    pub env: Vec<u8>,
    /// Envelope release
    ///
    /// TODO: Research how this works
    pub env_release: u32,
}

impl VoiceInstance {
    /// Recalculate the envelope based on the definition in `voice_unit`.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn recalc_envelope(&mut self, voice_unit: &VoiceUnit, out_sps: SampleRate) {
        let envelope = &(voice_unit).envelope;
        let Some((prepared, head)) = envelope.to_prepared(out_sps) else {
            return;
        };
        self.env = prepared;
        if head < envelope.points.len() {
            self.env_release = (f64::from((envelope.points[head]).x) * f64::from(out_sps)
                / f64::from(envelope.seconds_per_point)) as u32;
        } else {
            self.env_release = 0;
        }
    }
    /// Recalculate the sample buffer from [`WaveData`].
    pub fn recalc_wave_data(&mut self, wave: &WaveData, volume: i16, pan: i16) {
        self.num_samples = 400;
        let size = self.num_samples * 2 * 2;
        self.sample_buf = vec![0; size as usize];
        update_wave_ptv(wave, self, volume, pan);
    }
}

/// Convert relative envelope to absolute
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_absolute(envelope: &EnvelopeSrc, head: usize, out_sps: SampleRate) -> (Vec<(u32, u8)>, u32) {
    let mut points: Vec<(u32, u8)> = vec![(0, 0); head];

    let mut offset: u32 = 0;
    let mut head_num: u32 = 0;
    for (e, pt) in points.iter_mut().enumerate().take(head) {
        if e == 0 || (envelope.points[e]).x != 0 || (envelope.points[e]).y != 0 {
            offset += (f64::from((envelope.points[e]).x) * f64::from(out_sps)
                / f64::from(envelope.seconds_per_point)) as u32;
            pt.0 = offset;
            pt.1 = envelope.points[e].y;
            head_num += 1;
        }
    }
    (points, head_num)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn to_prepared_envelope(dst: &mut [u8], abs_points: &[(u32, u8)], head_num: u32) {
    let mut e = 0;
    let mut start: (u32, i32) = (0, 0);
    for (i, out) in dst.iter_mut().enumerate() {
        while (e as u32) < head_num && i as u32 >= abs_points[e].0 {
            start.0 = abs_points[e].0;
            start.1 = i32::from(abs_points[e].1);
            e += 1;
        }

        *out = if (e as u32) < head_num {
            (start.1
                + (i32::from(abs_points[e].1) - start.1) * (i as i32 - start.0 as i32)
                    / (abs_points[e].0 as i32 - start.0 as i32)) as u8
        } else {
            start.1 as u8
        }
    }
}

/// Describes an envelope for a [`Voice`].
///
/// This is used to generate [`VoiceInstance::env`].
#[derive(Clone, Default)]
pub struct EnvelopeSrc {
    /// The higher, the less envelope points there will be per second
    pub seconds_per_point: u32,
    /// Points of the envelope.
    ///
    /// X axis is time, Y axis is volume.
    ///
    /// Each point's X coordinate is an offset from the previous x coordinate, rather
    /// than an absolute position.
    pub points: Vec<EnvPt>,
}

impl EnvelopeSrc {
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn to_prepared(&self, out_sps: SampleRate) -> Option<(Vec<u8>, usize)> {
        if self.points.is_empty() {
            return None;
        }
        let mut size: u32 = 0;

        let head = self.points.len().saturating_sub(1);

        for e in 0..head {
            size += u32::from(self.points[e].x);
        }
        let env_samples_per_second = size * u32::from(out_sps);
        let mut env_size =
            (f64::from(env_samples_per_second) / f64::from(self.seconds_per_point)) as usize;
        if env_size == 0 {
            env_size = 1;
        }

        if env_size > ENV_SIZE_SAFETY_LIMIT {
            eprintln!("EnvelopeSrc::to_prepared: env_size too large ({env_size}).");
            return None;
        }

        let (abs_points, head_num) = to_absolute(self, head, out_sps);
        let mut prepared = vec![0; env_size];
        to_prepared_envelope(&mut prepared, &abs_points, head_num);
        Some((prepared, head))
    }
}

/// Data required to generate and play voice samples
#[derive(Clone)]
pub struct VoiceUnit {
    /// The native key of this voice. If not set correctly, the voice might sound
    /// off-key, or too low/high pitch when notes are being played with it.
    pub basic_key: Key,
    /// Volume to generate the samples with
    pub volume: i16,
    /// Panning
    pub pan: i16,
    /// Fine tuning of the pitch
    pub tuning: f32,
    /// Various properties of the voice that can be set
    pub flags: VoiceFlags,
    /// The data the voice samples are generated from
    pub data: VoiceData,
    /// The data the voice envelope is generated from
    pub envelope: EnvelopeSrc,
}

bitflags::bitflags! {
    /// Different attributes a voice can have
    #[derive(Clone, Copy, Default, bytemuck::AnyBitPattern, bytemuck::NoUninit, Debug)]
    #[repr(C)]
    pub struct VoiceFlags: u32 {
        /// Keep looping the voice instead of just playing it once
        const WAVE_LOOP = 0b001;
        /// Apply a smooth filter
        const SMOOTH    = 0b010;
        /// Make the frequency fit the beat tempo of the song
        const BEAT_FIT  = 0b100;
    }
}

/// Data keeping track of play state of a voice
#[derive(Default, Clone)]
pub struct VoiceTone {
    /// Keeps track of which sample of the voice we're currently at
    pub smp_pos: f64,
    /// How much to advance the sample position for the next sample
    pub offset_freq: f32,
    /// Keeps track of what the current volume is (sourced from [`VoiceInstance::env`])
    pub env_volume: u8,
    /// As long as this is greater than zero, the voice keeps playing
    pub life_count: i32,
    /// As long as this is greater than zero, the envelope position keeps incrementing
    pub on_count: i32,
    /// Plays into envelope volume calculation
    pub env_start: u8,
    /// Keeps track of the position in the envelope
    pub env_pos: usize,
    /// Presumably how long the "release" stage of the volume envelope should last.
    pub env_release_clock: u32,
}

/// Audio data that gives [`Unit`](crate::Unit)s a voice. In other words, an instrument.
pub struct Voice {
    /// Mostly static data required to generate the voice samples
    pub units: ArrayVec<VoiceUnit, 2>,
    /// Dynamic data to keep track of voice play state
    pub insts: ArrayVec<VoiceInstance, 2>,
    /// Name of the voice
    pub name: String,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            units: ArrayVec::default(),
            insts: ArrayVec::default(),
            name: "<no name>".into(),
        }
    }
}

impl Voice {
    pub(crate) fn tone_ready_sample(&mut self, ptn_bldr: &NoiseTable) {
        for (vinst, vunit) in zip(&mut self.insts, &mut self.units) {
            vinst.num_samples = 0;

            match &mut vunit.data {
                VoiceData::Pcm(pcm) => {
                    let (body, buf) = pcm.to_converted(NATIVE_SAMPLE_RATE);
                    vinst.num_samples = body;
                    vinst.sample_buf = buf;
                }

                VoiceData::Noise(ptn) => {
                    vinst.sample_buf = noise_to_pcm(ptn, ptn_bldr).into_sample_buf();
                    vinst.num_samples = ptn.smp_num_44k;
                }

                VoiceData::Wave(wave) => {
                    vinst.recalc_wave_data(wave, vunit.volume, vunit.pan);
                }
            }
        }
    }

    pub(crate) fn tone_ready_envelopes(&mut self, sps: SampleRate) {
        for (voice_inst, voice_unit) in zip(&mut self.insts, &self.units) {
            voice_inst.recalc_envelope(voice_unit, sps);
        }
    }

    pub(crate) fn tone_ready(&mut self, ptn_bldr: &NoiseTable, out_sps: SampleRate) {
        self.tone_ready_sample(ptn_bldr);
        self.tone_ready_envelopes(out_sps);
    }
    /// Allocate voice unit for either a single channel, or both.
    pub fn allocate<const BOTH: bool>(&mut self) {
        let u = VoiceUnit {
            basic_key: DEFAULT_BASICKEY.cast_signed(),
            tuning: 1.0,
            flags: VoiceFlags::SMOOTH,
            envelope: EnvelopeSrc::default(),
            data: VoiceData::Noise(NoiseData::new()),
            volume: 0,
            pan: 0,
        };
        self.units.push(u.clone());
        self.insts.push(VoiceInstance::default());
        if BOTH {
            self.units.push(u);
            self.insts.push(VoiceInstance::default());
        }
    }
}

// Never allocate an envelope larger than this (1 megabyte)
const ENV_SIZE_SAFETY_LIMIT: usize = 1_048_576;

fn update_wave_ptv(wave: &WaveData, inst: &mut VoiceInstance, volume: i16, pan: i16) {
    let mut pan_volume: [i16; 2] = [64, 64];

    if pan > 64 {
        pan_volume[0] = 128 - pan;
    }
    if pan < 64 {
        pan_volume[1] = pan;
    }

    let osci = OsciArgs {
        volume,
        sample_num: inst.num_samples,
    };

    let smp_buf_16: &mut [i16] = bytemuck::cast_slice_mut(&mut inst.sample_buf[..]);
    for s in 0..inst.num_samples {
        let osc = match wave {
            WaveData::Coord {
                points: coordinates,
                resolution,
            } => coord(osci, coordinates, s.try_into().unwrap(), *resolution),
            WaveData::Overtone {
                points: coordinates,
            } => overtone(osci, coordinates, s.try_into().unwrap()),
        };
        for c in 0..2 {
            let mut work = osc * f64::from(pan_volume[c]) / 64.;
            work = work.clamp(-1.0, 1.0);
            #[expect(clippy::cast_possible_truncation)]
            (smp_buf_16[s as usize * 2 + c] = (work * 32767.) as i16);
        }
    }
}
