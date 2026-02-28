use encoding_rs::SHIFT_JIS;

use crate::{
    delay::{Delay, DelayUnit},
    event::EveList,
    herd::{
        Delays, FmtInfo, FmtKind, FmtVer, Herd, MAX_TUNE_UNIT_NAME, MAX_TUNE_VOICE_NAME, MAX_UNITS,
        MooInstructions, Song,
    },
    io::{ReadError, Reader},
    master::Master,
    overdrive::Overdrive,
    result::{ProjectReadError, ProjectWriteError, ReadResult, WriteResult},
    unit::{GroupIdx, Unit},
    voice::Voice,
};

type Code = [u8; CODESIZE];

pub enum Tag {
    AntiOPER,
    V1Proj,
    V1Unit,
    V1Pcm,
    V1Event,
    V1End,
    V3Unit,
    V4EvenMast,
    V4EvenUnit,
    NumUNIT,
    MasterV5,
    EventV5,
    MatePCM,
    MatePTV,
    MatePTN,
    MateOGGV,
    EffeDELA,
    EffeOVER,
    TextNAME,
    TextCOMM,
    AssiUNIT,
    AssiWOIC,
    PxtoneND,
}

const VERSIONSIZE: usize = 16;
const CODESIZE: usize = 8;

fn read_tune_items(
    song: &mut Song,
    herd: &mut Herd,
    ins: &mut MooInstructions,
    rd: &mut Reader,
) -> ReadResult {
    let mut end = false;

    while !end {
        let code = rd.next::<Code>()?;

        let Some(tag) = Tag::from_code(code) else {
            return Err(ProjectReadError::FmtUnknown);
        };
        match tag {
            Tag::AntiOPER => {
                return Err(ProjectReadError::AntiOpreation);
            }
            Tag::NumUNIT => {
                let num = read_unit_num(rd)?;
                for _ in 0..num {
                    herd.units.push(Unit::new());
                }
            }

            Tag::MasterV5 => {
                song.master = Master::read_v5(rd)?;
            }
            Tag::EventV5 => {
                song.events = EveList::read(rd)?;
            }

            Tag::MatePCM | Tag::V1Pcm => {
                read_voice(ins, rd, IoVoiceType::Pcm)?;
            }
            Tag::MatePTV => {
                read_voice(ins, rd, IoVoiceType::Ptv)?;
            }
            Tag::MatePTN => {
                read_voice(ins, rd, IoVoiceType::Ptn)?;
            }

            Tag::MateOGGV => {
                read_voice(ins, rd, IoVoiceType::Oggv)?;
            }

            Tag::EffeDELA => {
                read_delay(rd, &mut herd.delays)?;
            }
            Tag::EffeOVER => {
                herd.overdrives.push(read_overdrive(rd)?);
            }
            Tag::TextNAME => {
                song.text.name_r(rd)?;
            }
            Tag::TextCOMM => {
                song.text.comment_r(rd)?;
            }
            Tag::AssiWOIC => {
                read_assist_voice(rd, ins)?;
            }
            Tag::AssiUNIT => {
                read_unit(herd, rd)?;
            }
            Tag::PxtoneND | Tag::V1End => {
                end = true;
            }
            Tag::V4EvenMast
            | Tag::V4EvenUnit
            | Tag::V3Unit
            | Tag::V1Proj
            | Tag::V1Unit
            | Tag::V1Event => {
                return Err(ProjectReadError::OldUnsupported);
            }
        }
    }

    Ok(())
}

fn write_tune_items(
    out: &mut Vec<u8>,
    song: &Song,
    herd: &Herd,
    ins: &MooInstructions,
) -> WriteResult<()> {
    out.extend_from_slice(Tag::MasterV5.to_code());
    song.master.write_v5(out);
    out.extend_from_slice(Tag::EventV5.to_code());
    song.events.write(out);
    song.text.name_w(out);
    song.text.comment_w(out);
    for delay in &herd.delays {
        out.extend_from_slice(Tag::EffeDELA.to_code());
        write_delay(delay, out);
    }
    for ovr in &herd.overdrives {
        out.extend_from_slice(Tag::EffeOVER.to_code());
        write_overdrive(ovr, out);
    }
    for (i, voice) in ins.voices.iter().enumerate() {
        write_voice(voice, i, out)?;
    }
    write_unit_num(out, herd);
    write_units(out, herd);
    Ok(())
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::AnyBitPattern, bytemuck::NoUninit)]
struct IoDelay {
    unit: u16,
    group: u16,
    rate: f32,
    freq: f32,
}

fn read_delay(rd: &mut Reader, delays: &mut Delays) -> ReadResult {
    let size: u32 = rd.next()?;
    if size as usize != size_of::<IoDelay>() {
        return Err(ProjectReadError::FmtUnknown);
    }
    let io_delay: IoDelay = rd.next()?;
    let unit = match io_delay.unit {
        0 => DelayUnit::Beat,
        1 => DelayUnit::Meas,
        2 => DelayUnit::Second,
        _ => return Err(ProjectReadError::FmtUnknown),
    };
    let delay = Delay {
        unit,
        group: GroupIdx(io_delay.group.try_into().unwrap()),
        // The rate is effectively an integer, but stored as float in the PxTone format
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        rate: io_delay.rate as u8,
        freq: io_delay.freq,
        offset: 0,
        bufs: [Vec::new(), Vec::new()],
    };
    delays.push(delay);
    Ok(())
}

fn write_delay(delay: &Delay, out: &mut Vec<u8>) {
    let size: u32 = size_of::<IoDelay>().try_into().unwrap();
    out.extend_from_slice(&size.to_le_bytes());
    let unit = match delay.unit {
        DelayUnit::Beat => 0,
        DelayUnit::Meas => 1,
        DelayUnit::Second => 2,
    };
    let io_delay = IoDelay {
        unit,
        group: u16::from(delay.group.0),
        rate: f32::from(delay.rate),
        freq: delay.freq,
    };
    out.extend_from_slice(bytemuck::bytes_of(&io_delay));
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::AnyBitPattern, bytemuck::NoUninit)]
struct IoOverDrv {
    xxx: u16,
    group: u16,
    cut: f32,
    amp: f32,
    yyy: f32,
}

fn read_overdrive(rd: &mut Reader) -> ReadResult<Overdrive> {
    let _size: u32 = rd.next().unwrap();
    let ovr: IoOverDrv = rd.next().unwrap();
    if ovr.xxx != 0 {
        return Err(ProjectReadError::FmtUnknown);
    }
    if ovr.xxx != 0 {
        return Err(ProjectReadError::FmtUnknown);
    }
    if !Overdrive::CUT_VALID_RANGE.contains(&ovr.cut) {
        return Err(ProjectReadError::FmtUnknown);
    }
    if !Overdrive::AMP_VALID_RANGE.contains(&ovr.amp) {
        return Err(ProjectReadError::FmtUnknown);
    }
    Ok(Overdrive {
        cut_percent: ovr.cut,
        amp_mul: ovr.amp,
        group: GroupIdx(ovr.group.try_into().unwrap()),
        on: true,
        cut_16bit_top: 0,
    })
}

fn write_overdrive(ovr: &Overdrive, out: &mut Vec<u8>) {
    let size: u32 = size_of::<IoOverDrv>().try_into().unwrap();
    out.extend_from_slice(&size.to_le_bytes());
    let io_ovr = IoOverDrv {
        xxx: 0,
        group: u16::from(ovr.group.0),
        cut: ovr.cut_percent,
        amp: ovr.amp_mul,
        yyy: 0.0,
    };
    out.extend_from_slice(bytemuck::bytes_of(&io_ovr));
}

#[derive(Clone, Copy)]
enum IoVoiceType {
    Pcm,
    Ptv,
    Ptn,
    Oggv,
}

impl Tag {
    const fn from_code(code: Code) -> Option<Self> {
        Some(match &code {
            b"antiOPER" => Self::AntiOPER,
            b"assiUNIT" => Self::AssiUNIT,
            b"assiWOIC" => Self::AssiWOIC,
            b"effeDELA" => Self::EffeDELA,
            b"effeOVER" => Self::EffeOVER,
            b"Event V5" => Self::EventV5,
            b"MasterV5" => Self::MasterV5,
            b"mateOGGV" => Self::MateOGGV,
            b"matePCM " => Self::MatePCM,
            b"matePTN " => Self::MatePTN,
            b"matePTV " => Self::MatePTV,
            b"num UNIT" => Self::NumUNIT,
            b"pxtoneND" => Self::PxtoneND,
            b"textCOMM" => Self::TextCOMM,
            b"textNAME" => Self::TextNAME,
            b"END=====" => Self::V1End,
            b"EVENT===" => Self::V1Event,
            b"matePCM=" => Self::V1Pcm,
            b"PROJECT=" => Self::V1Proj,
            b"UNIT====" => Self::V1Unit,
            b"pxtnUNIT" => Self::V3Unit,
            b"evenMAST" => Self::V4EvenMast,
            b"evenUNIT" => Self::V4EvenUnit,
            _ => return None,
        })
    }
    pub const fn to_code(&self) -> &'static Code {
        match self {
            Self::AntiOPER => b"antiOPER",
            Self::AssiUNIT => b"assiUNIT",
            Self::AssiWOIC => b"assiWOIC",
            Self::EffeDELA => b"effeDELA",
            Self::EffeOVER => b"effeOVER",
            Self::EventV5 => b"Event V5",
            Self::MasterV5 => b"MasterV5",
            Self::MateOGGV => b"mateOGGV",
            Self::MatePCM => b"matePCM ",
            Self::MatePTN => b"matePTN ",
            Self::MatePTV => b"matePTV ",
            Self::NumUNIT => b"num UNIT",
            Self::PxtoneND => b"pxtoneND",
            Self::TextCOMM => b"textCOMM",
            Self::TextNAME => b"textNAME",
            Self::V1End => b"END=====",
            Self::V1Event => b"EVENT===",
            Self::V1Pcm => b"matePCM=",
            Self::V1Proj => b"PROJECT=",
            Self::V1Unit => b"UNIT====",
            Self::V3Unit => b"pxtnUNIT",
            Self::V4EvenMast => b"evenMAST",
            Self::V4EvenUnit => b"evenUNIT",
        }
    }
}

#[derive(bytemuck::AnyBitPattern, bytemuck::NoUninit, Clone, Copy)]
#[repr(C)]
struct IoUnit {
    unit_index: u16,
    rrr: u16,
    name: [u8; MAX_TUNE_UNIT_NAME],
}

fn read_unit(herd: &mut Herd, rd: &mut Reader) -> ReadResult {
    let size = rd.next::<u32>()?;

    if size as usize != size_of::<IoUnit>() {
        return Err(ProjectReadError::FmtUnknown);
    }

    let io_unit = rd.next::<IoUnit>()?;
    if io_unit.rrr != 0 {
        return Err(ProjectReadError::FmtUnknown);
    }
    // Max number of units is 50, yet the field is 16 bits, so if it can't be converted, we bail
    let unit_idx: u8 = match io_unit.unit_index.try_into() {
        Ok(idx) => idx,
        Err(_) => return Err(ProjectReadError::FmtUnknown),
    };
    if unit_idx >= herd.units.len() {
        return Err(ProjectReadError::FmtUnknown);
    }

    let len = strlen(&io_unit.name) as usize;

    herd.units[unit_idx].name = SHIFT_JIS.decode(&io_unit.name[..len]).0.into_owned();

    Ok(())
}

fn write_units(out: &mut Vec<u8>, herd: &Herd) {
    for (i, unit) in herd.units.iter().take(MAX_UNITS as usize).enumerate() {
        // TODO: Fix this no name thingy? Maybe Option?
        if unit.name == "<no name>" {
            continue;
        }
        out.extend_from_slice(Tag::AssiUNIT.to_code());
        let size: u32 = size_of::<IoUnit>().try_into().unwrap();
        out.extend_from_slice(&size.to_le_bytes());
        let shift_jis = SHIFT_JIS.encode(&unit.name);
        let mut name: [u8; MAX_TUNE_UNIT_NAME] = [0; _];
        let max_len = std::cmp::min(shift_jis.0.len(), MAX_TUNE_UNIT_NAME);
        name[..max_len].copy_from_slice(&shift_jis.0[..max_len]);
        let io_unit = IoUnit {
            unit_index: i.try_into().unwrap(),
            rrr: 0,
            name,
        };
        out.extend_from_slice(bytemuck::bytes_of(&io_unit));
    }
}

#[expect(clippy::cast_possible_truncation)]
fn strlen(buf: &[u8]) -> u8 {
    buf.iter().position(|b| *b == 0).unwrap_or(MAX_TUNE_UNIT_NAME) as u8
}

const V1_COLLAGE: &[u8; VERSIONSIZE] = b"PTCOLLAGE-050227";
const V2_COLLAGE: &[u8; VERSIONSIZE] = b"PTCOLLAGE-050608";
const V2_TUNE: &[u8; VERSIONSIZE] = b"PTTUNE--20050608";
const V3_COLLAGE: &[u8; VERSIONSIZE] = b"PTCOLLAGE-060115";
const V3_TUNE: &[u8; VERSIONSIZE] = b"PTTUNE--20060115";
const V4_COLLAGE: &[u8; VERSIONSIZE] = b"PTCOLLAGE-060930";
const V4_TUNE: &[u8; VERSIONSIZE] = b"PTTUNE--20060930";
const V5_COLLAGE: &[u8; VERSIONSIZE] = b"PTCOLLAGE-071119";
const V5_TUNE: &[u8; VERSIONSIZE] = b"PTTUNE--20071119";

fn read_version(rd: &mut Reader) -> ReadResult<FmtInfo> {
    let version = rd.next::<[u8; VERSIONSIZE]>()?;

    let (fmt_ver, fmt_kind) = match &version {
        V1_COLLAGE => (FmtVer::V1, FmtKind::Collage),
        V2_COLLAGE => (FmtVer::V2, FmtKind::Collage),
        V2_TUNE => (FmtVer::V2, FmtKind::Tune),
        V3_COLLAGE => (FmtVer::V3, FmtKind::Collage),
        V3_TUNE => (FmtVer::V3, FmtKind::Tune),
        V4_COLLAGE => (FmtVer::V4, FmtKind::Collage),
        V4_TUNE => (FmtVer::V4, FmtKind::Tune),
        V5_COLLAGE => (FmtVer::V5, FmtKind::Collage),
        V5_TUNE => (FmtVer::V5, FmtKind::Tune),
        _ => {
            return Err(ProjectReadError::FmtUnknown);
        }
    };

    let exe_ver = rd.next::<u16>()?;
    let dummy = rd.next::<u16>()?;

    Ok(FmtInfo {
        ver: fmt_ver,
        kind: fmt_kind,
        exe_ver,
        dummy,
    })
}

fn read_voice(ins: &mut MooInstructions, rd: &mut Reader, kind: IoVoiceType) -> ReadResult {
    let voice = match kind {
        IoVoiceType::Pcm => Voice::read_mate_pcm(rd)?,
        IoVoiceType::Ptv => Voice::read_mate_ptv(rd)?,
        IoVoiceType::Ptn => Voice::read_mate_ptn(rd)?,
        IoVoiceType::Oggv => Voice::read_ogg(rd)?,
    };
    ins.voices.push(voice);
    Ok(())
}

fn write_voice(voice: &Voice, idx: usize, out: &mut Vec<u8>) -> WriteResult {
    match &voice.base.data {
        crate::VoiceData::Noise(noise_data) => voice.write_mate_ptn(out, noise_data),
        // TODO: Ogg/vorbis is being serialized as PCM (because we also deserialize it as such)
        crate::VoiceData::Pcm(pcm_data) => voice.write_mate_pcm(out, pcm_data),
        crate::VoiceData::Wave(_wave_data) => voice.write_mate_ptv(out)?,
        crate::VoiceData::OggV(oggv_data) => voice.write_mate_oggv(out, oggv_data),
    }
    // TODO: Fix this no name thingy?
    if voice.name != "<no name>" {
        write_assist_voice(voice, idx, out);
    }
    Ok(())
}

#[derive(Default, bytemuck::AnyBitPattern, Clone, Copy)]
struct NumUnit {
    num: u16,
    rrr: u16,
}

fn read_unit_num(rd: &mut Reader) -> ReadResult<i32> {
    let size = rd.next::<u32>()?;
    if size as usize != size_of::<NumUnit>() {
        return Err(ProjectReadError::FmtUnknown);
    }
    let data = rd.next::<NumUnit>()?;
    if data.rrr != 0 {
        return Err(ProjectReadError::FmtUnknown);
    }
    if data.num > MAX_UNITS {
        return Err(ProjectReadError::FmtNewer);
    }

    Ok(i32::from(data.num))
}

fn write_unit_num(out: &mut Vec<u8>, herd: &Herd) {
    out.extend_from_slice(Tag::NumUNIT.to_code());
    let size: u32 = size_of::<NumUnit>().try_into().unwrap();
    out.extend_from_slice(&size.to_le_bytes());
    let mut n_units: u16 = herd.units.len().into();
    // Only 50 units are supported by the serialization format
    if n_units > MAX_UNITS {
        n_units = MAX_UNITS;
    }
    out.extend_from_slice(&n_units.to_le_bytes());
    let rrr: u16 = 0;
    out.extend_from_slice(&rrr.to_le_bytes());
}

#[derive(bytemuck::AnyBitPattern, bytemuck::NoUninit, Clone, Copy)]
#[repr(C)]
struct AssistVoice {
    voice_idx: u16,
    rrr: u16,
    name: [u8; MAX_TUNE_VOICE_NAME as usize],
}

fn read_assist_voice(rd: &mut Reader, ins: &mut MooInstructions) -> ReadResult {
    let size = rd.next::<u32>()?;
    if size as usize != size_of::<AssistVoice>() {
        return Err(ProjectReadError::FmtUnknown);
    }
    let assi = rd.next::<AssistVoice>()?;

    if assi.rrr != 0 {
        eprintln!("Warning: rrr is not 0. Possibly invalid.");
    }

    // The maximum number of voices is 100, so the voice number must fit into u8
    let Ok(idx) = u8::try_from(assi.voice_idx) else {
        return Err(ProjectReadError::FmtUnknown);
    };

    let voice = &mut ins.voices[crate::VoiceIdx(idx)];
    let len = strlen(&assi.name);
    voice.name = SHIFT_JIS.decode(&assi.name[..len as usize]).0.into_owned();

    Ok(())
}

fn write_assist_voice(voice: &Voice, idx: usize, out: &mut Vec<u8>) {
    out.extend_from_slice(Tag::AssiWOIC.to_code());
    let size: u32 = size_of::<AssistVoice>().try_into().unwrap();
    out.extend_from_slice(&size.to_le_bytes());
    let mut name: [u8; MAX_TUNE_VOICE_NAME as usize] = [0; _];
    let shift_jis = SHIFT_JIS.encode(&voice.name).0;
    name[..shift_jis.len()].copy_from_slice(&shift_jis);
    let assi = AssistVoice {
        voice_idx: idx.try_into().unwrap(),
        rrr: 0,
        name,
    };
    out.extend_from_slice(bytemuck::bytes_of(&assi));
}

pub(super) fn read(
    song: &mut Song,
    herd: &mut Herd,
    ins: &mut MooInstructions,
    data: &[u8],
) -> ReadResult {
    let mut reader = Reader { data, cur: 0 };
    reader.cur = 0;
    song.fmt = read_version(&mut reader)?;
    read_tune_items(song, herd, ins, &mut reader)?;
    Ok(())
}

pub(super) fn write(song: &Song, herd: &Herd, ins: &MooInstructions) -> WriteResult<Vec<u8>> {
    let mut out = Vec::new();
    write_version(&mut out, song.fmt)?;
    write_tune_items(&mut out, song, herd, ins)?;
    out.extend_from_slice(Tag::PxtoneND.to_code());
    // Tail zero bytes (dummy tag value?)
    out.extend_from_slice(&[0; 4]);
    Ok(out)
}

fn write_version(out: &mut Vec<u8>, info: FmtInfo) -> WriteResult {
    let bytes = match (info.ver, info.kind) {
        (FmtVer::V1, FmtKind::Collage) => V1_COLLAGE,
        (FmtVer::V1, FmtKind::Tune) => return Err(ProjectWriteError::UnsupportedFmt),
        (FmtVer::V2, FmtKind::Collage) => V2_COLLAGE,
        (FmtVer::V2, FmtKind::Tune) => V2_TUNE,
        (FmtVer::V3, FmtKind::Collage) => V3_COLLAGE,
        (FmtVer::V3, FmtKind::Tune) => V3_TUNE,
        (FmtVer::V4, FmtKind::Collage) => V4_COLLAGE,
        (FmtVer::V4, FmtKind::Tune) => V4_TUNE,
        (FmtVer::V5, FmtKind::Collage) => V5_COLLAGE,
        (FmtVer::V5, FmtKind::Tune) => V5_TUNE,
    };
    out.extend_from_slice(bytes);
    out.extend_from_slice(&info.exe_ver.to_le_bytes());
    out.extend_from_slice(&info.dummy.to_le_bytes());
    Ok(())
}

impl super::Text {
    pub(crate) fn comment_r(&mut self, rd: &mut Reader) -> ReadResult {
        self.comment = SHIFT_JIS.decode(&read_vec(rd)?).0.into_owned();
        Ok(())
    }

    pub(crate) fn comment_w(&self, out: &mut Vec<u8>) {
        if !self.comment.is_empty() {
            out.extend_from_slice(Tag::TextCOMM.to_code());
            write_shift_jis(&self.comment, out);
        }
    }

    pub(crate) fn name_r(&mut self, rd: &mut Reader) -> ReadResult {
        self.name = SHIFT_JIS.decode(&read_vec(rd)?).0.into_owned();
        Ok(())
    }

    pub(crate) fn name_w(&self, out: &mut Vec<u8>) {
        if !self.name.is_empty() {
            out.extend_from_slice(Tag::TextNAME.to_code());
            write_shift_jis(&self.name, out);
        }
    }
}

fn write_shift_jis(text: &str, out: &mut Vec<u8>) {
    let shift_jis = SHIFT_JIS.encode(text);
    // We assume we don't have text >4GB
    #[expect(clippy::cast_possible_truncation)]
    let len: u32 = shift_jis.0.len() as u32;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&shift_jis.0);
}

fn read_vec(rd: &mut Reader) -> Result<Vec<u8>, ReadError> {
    let size = rd.next::<u32>()?;
    let mut v: Vec<u8> = vec![0; size as usize];
    rd.fill_slice(&mut v)?;
    Ok(v)
}
