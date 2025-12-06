use crate::{
    ReadResult, SampleRate, SamplesPerTick, Timing, delay::Delay, event::EveList, master::Master,
    noise_builder::NoiseTable, overdrive::Overdrive, result::WriteResult, timing::SampleT,
    unit::Unit, voice::Voice,
};

mod io;
pub use io::Tag;
pub mod moo;

const MAX_UNITS: u16 = 50;
const MAX_TUNE_VOICE_NAME: u32 = 16;
pub const MAX_TUNE_UNIT_NAME: usize = 16;

/// Song name and comment
#[derive(Default)]
pub struct Text {
    /// Name of the song
    pub name: String,
    /// Comment (short description) for the song
    pub comment: String,
}

/// PxTone format version
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum FmtVer {
    /// Version 1
    V1,
    /// Version 2
    V2,
    /// Version 3
    V3,
    /// Version 4
    V4,
    /// Version 5
    V5,
}

/// Kind of PxTone format we are dealing with
#[derive(Clone, Copy, Debug)]
pub enum FmtKind {
    /// PxTone collage (.ptcop)
    Collage,
    /// PxTone tune (.pttune)
    Tune,
}

/// Information about what format the song is
#[derive(Clone, Copy, Debug)]
pub struct FmtInfo {
    /// PxTone format version
    pub ver: FmtVer,
    /// Are we a project or a tune?
    pub kind: FmtKind,
    pub(crate) exe_ver: u16,
    pub(crate) dummy: u16,
}

impl Default for FmtInfo {
    fn default() -> Self {
        Self {
            ver: FmtVer::V5,
            kind: FmtKind::Collage,
            exe_ver: 0,
            dummy: 0,
        }
    }
}

/// A PxTone song
#[derive(Default)]
pub struct Song {
    /// The name and the comment of the song
    pub text: Text,
    /// Contains timing related data and the loop points of the song
    pub master: Master,
    /// The events of the song
    pub events: EveList,
    /// Information about the pxtone file format this song has
    pub fmt: FmtInfo,
}

/// How to moo the song
pub struct MooInstructions {
    /// Output sample rate
    pub out_sample_rate: SampleRate,
    /// The voices of the cows
    pub voices: Vec<Voice>,
    /// How many samples constitute a tick.
    pub samples_per_tick: SamplesPerTick,
}

impl MooInstructions {
    /// Create a new [`MooInstructions`] with the provided sample rate
    #[must_use]
    pub const fn new(out_sample_rate: SampleRate) -> Self {
        Self {
            out_sample_rate,
            voices: Vec::new(),
            samples_per_tick: 1.0,
        }
    }
}

/// Adjust voice and effect tones to output sample rate
pub fn rebuild_tones(
    ins: &mut MooInstructions,
    out_sample_rate: SampleRate,
    delays: &mut [Delay],
    overdrives: &mut [Overdrive],
    master: &Master,
) {
    for delay in delays {
        delay.rebuild(
            master.timing.beats_per_meas,
            master.timing.bpm,
            ins.out_sample_rate,
        );
    }
    for ovr in overdrives {
        ovr.rebuild();
    }
    let builder = NoiseTable::generate();
    for voice in &mut ins.voices {
        voice.tone_ready(&builder, out_sample_rate);
    }
}

/// The glorious cows that are going to moo your song
#[derive(Default)]
pub struct Herd {
    end: bool,
    loop_: bool,
    smp_smooth: SampleRate,
    /// Counter variable for what sample we are at
    pub smp_count: SampleT,
    smp_start: SampleT,
    /// The song will end at this sample
    pub smp_end: SampleT,
    /// The song will repeat from here
    pub smp_repeat: SampleT,
    smp_stride: f32,
    time_pan_index: usize,
    /// What event to play next
    pub evt_idx: usize,
    /// The 🐄 cow units that drive music synthesis. Each one outputs a PCM stream that's mixed
    /// together for a final result.
    pub units: Vec<Unit>,
    /// Delay (reverb) effects
    pub delays: Vec<Delay>,
    /// Overdrive (amplify + clip) effects
    pub overdrives: Vec<Overdrive>,
}

impl Herd {
    /// Seek to sample count
    pub const fn seek_to_sample(&mut self, sample: SampleT) {
        self.smp_count = sample;
        // If we set the event index to zero, the correct event index will be found when we moo
        self.evt_idx = 0;
    }
    /// Make sure all the cows' voices are ready for playback
    pub fn tune_cow_voices(&mut self, ins: &MooInstructions, timing: Timing) {
        for unit in &mut self.units {
            unit.tone_init();
            unit.reset_voice(ins, 0, timing);
        }
    }
}

/// Read a PxTone song from a byte array.
///
/// Returns a tuple of:
/// - The [`Song`]: Mostly static song data that doesn't change during playback
/// - The [`Herd`]: The cows (units), and other data that keeps track of playback state
/// - The [`MooInstructions`]: Contains the [`Voice`]s of the cows, and some other data required
///   for mooing.
///
/// The current organization structure is a bit arbitrary, reached after a lot of refactoring
/// of various parts of the codebase. It will probably change in future releases to a cleaner API.
///
/// ## Playback
///
/// If your goal is to play the song, you should call [`moo_prepare`](crate::moo_prepare) next,
/// after which you can get samples to output with [`Herd::moo`].
#[expect(clippy::missing_errors_doc)]
pub fn read_song(
    data: &[u8],
    out_sample_rate: SampleRate,
) -> ReadResult<(Song, Herd, MooInstructions)> {
    let mut song = Song {
        text: Text::default(),
        master: Master::default(),
        events: EveList::default(),
        fmt: FmtInfo {
            ver: FmtVer::V5,
            kind: FmtKind::Collage,
            exe_ver: 0,
            dummy: 0,
        },
    };
    let mut ins = MooInstructions {
        out_sample_rate,
        voices: Vec::new(),
        samples_per_tick: 0.0,
    };
    let mut herd = Herd::default();

    io::read(&mut song, &mut herd, &mut ins, data)?;
    song.master.adjust_meas_num(std::cmp::max(
        song.master.get_last_tick(),
        song.events.get_max_tick(),
    ));
    rebuild_tones(
        &mut ins,
        out_sample_rate,
        &mut herd.delays,
        &mut herd.overdrives,
        &song.master,
    );
    Ok((song, herd, ins))
}

/// Serialize the project into the PxTone file format
pub fn serialize_project(song: &Song, herd: &Herd, ins: &MooInstructions) -> WriteResult<Vec<u8>> {
    io::write(song, herd, ins)
}
