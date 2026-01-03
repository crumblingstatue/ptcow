#![doc = include_str!("../README.md")]
// When we return an error type, the possible errors are encoded within it.
#![allow(clippy::missing_errors_doc)]

mod delay;
mod event;
mod herd;
mod io;
mod master;
mod noise_builder;
mod overdrive;
mod point;
mod pulse_frequency;
mod pulse_oscillator;
mod result;
pub mod timing;
mod unit;
mod voice;

mod voice_data {
    pub mod noise;
    pub mod oggv;
    pub mod pcm;
    pub mod wave;
}

mod util {
    mod array_len_ext;
    pub use array_len_ext::ArrayLenExt;
}

pub use {
    delay::{Delay, DelayUnit},
    event::{DEFAULT_KEY, EveList, Event, EventPayload, Key},
    herd::{
        FmtInfo, FmtKind, FmtVer, Herd, MooInstructions, Song, Text,
        moo::{MooPlan, StartPosPlan, current_tick, do_event, moo_prepare},
        read_song, rebuild_tones, serialize_project,
    },
    master::{LoopPoints, Master},
    noise_builder::{NoiseDesignOscillator, NoiseTable, NoiseType, noise_to_pcm},
    overdrive::Overdrive,
    point::EnvPt,
    pulse_oscillator::{OsciArgs, OsciPt},
    pulse_oscillator::{coord, overtone},
    result::{ProjectReadError, ReadResult},
    timing::{Meas, SampleT, SamplesPerTick, Tick, Tick16, Timing},
    unit::{GroupIdx, PanTimeBuf, PanTimeOff, Unit, UnitIdx},
    voice::{EnvelopeSrc, Voice, VoiceData, VoiceFlags, VoiceInstance, VoiceTone, VoiceUnit},
    voice_data::{
        noise::{NoiseData, NoiseDesignUnit},
        oggv::OggVData,
        pcm::PcmData,
        wave::WaveData,
    },
};

/// Channel number (mono or stereo)
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChNum {
    /// Monaural, one channel
    #[default]
    Mono = 1,
    /// Stereo, two channels
    Stereo = 2,
}

/// Bits per sample
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Bps {
    /// 8 bits per sample
    #[default]
    B8 = 8,
    /// 16 bits per sample (little endian)
    B16 = 16,
}

/// Wide enough to represent 48 Khz
pub type SampleRate = u16;
/// Some sources (Ogg/Vorbis) can have high sample rates (96 khz for example)
pub type SourceSampleRate = u32;
/// The sample rate `PxTone` internally works with
pub const NATIVE_SAMPLE_RATE: SampleRate = 44_100;

#[cfg(target_endian = "big")]
const _: () = panic!("Only little endian architectures are supported currently.");
