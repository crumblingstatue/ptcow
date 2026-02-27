use arrayvec::ArrayVec;

use crate::{
    Bps, ChNum, SampleRate, VoiceData, VoiceUnit,
    herd::Tag,
    io::write_varint,
    point::EnvPt,
    pulse_oscillator::OsciPt,
    result::{ProjectReadError, ProjectWriteError, ReadResult, WriteResult},
    voice::{EnvelopeSrc, Voice, VoiceFlags, VoiceSlot},
    voice_data::{noise::NoiseData, oggv::OggVData, pcm::PcmData, wave::WaveData},
};

#[cfg(feature = "oggv")]
mod oggv;

#[derive(Default, bytemuck::AnyBitPattern, Clone, Copy)]
#[repr(C)]
struct IoPcm {
    x3x_unit_no: u16,
    basic_key: u16,
    voice_flags: VoiceFlags,
    ch: u8,
    bps: u16,
    sps: SampleRate,
    tuning: f32,
    data_size: u32,
}

#[derive(Default, bytemuck::AnyBitPattern, bytemuck::NoUninit, Clone, Copy)]
#[repr(C)]
struct IoPtn {
    x3x_unit_no: u16,
    basic_key: u16,
    voice_flags: VoiceFlags,
    tuning: f32,
    rrr: i32,
}

/// I/O
impl Voice {
    pub(crate) fn read_mate_pcm(rd: &mut crate::io::Reader) -> ReadResult<Self> {
        let _size = rd.next::<u32>()?;

        let pcm = rd.next::<IoPcm>()?;

        let bps = match pcm.bps {
            8 => Bps::B8,
            16 => Bps::B16,
            _ => return Err(ProjectReadError::FmtUnknown),
        };

        let chnum = match pcm.ch {
            1 => ChNum::Mono,
            2 => ChNum::Stereo,
            _ => return Err(ProjectReadError::FmtUnknown),
        };
        let mut pcm_data = PcmData::new();
        pcm_data.create(
            chnum,
            pcm.sps.into(),
            bps,
            pcm.data_size / u32::from(bps as u16 / 8 * u16::from(pcm.ch)),
        );
        let smp_buf = pcm_data.sample_mut();
        rd.fill_slice(smp_buf)?;
        let vu = VoiceUnit {
            data: VoiceData::Pcm(pcm_data),
            flags: pcm.voice_flags,
            basic_key: i32::from(pcm.basic_key),
            tuning: pcm.tuning,
            ..Default::default()
        };
        Ok(Self::from_unit(vu))
    }

    pub(crate) fn write_mate_pcm(&self, out: &mut Vec<u8>, data: &PcmData) {
        out.extend_from_slice(Tag::MatePCM.to_code());
        #[expect(clippy::cast_possible_truncation)]
        let io_size: u32 = size_of::<IoPcm>() as u32 + data.smp.len() as u32;
        out.extend_from_slice(&io_size.to_le_bytes());
        let vu = &self.slots[0].unit;
        let io_pcm = IoPcm {
            x3x_unit_no: 0,
            basic_key: vu.basic_key.try_into().unwrap(),
            voice_flags: vu.flags,
            ch: data.ch as _,
            bps: data.bps as _,
            // TODO: Normally this assumption shouldn't be violated, but ogg voices
            // can have higher sps than what can fit into 16 bits.
            //
            // The fix for that is to not load ogg voices as pcm voices, but to properly load them as
            // ogg voices, and serialize them as such, rather than as pcm.
            sps: data.sps.try_into().unwrap(),
            tuning: vu.tuning,
            data_size: data.smp.len().try_into().unwrap(),
        };
        let mut io_pcm_byte_buf: Vec<u8> = vec![0; size_of::<IoPcm>()];
        // Safety: YOLO
        unsafe {
            std::ptr::copy(
                (&raw const io_pcm).cast(),
                io_pcm_byte_buf.as_mut_ptr(),
                size_of::<IoPcm>(),
            );
        }
        out.extend_from_slice(&io_pcm_byte_buf);
        out.extend_from_slice(&data.smp);
    }

    pub(crate) fn read_mate_ptn(rd: &mut crate::io::Reader) -> ReadResult<Self> {
        let _size = rd.next::<u32>()?;
        let ptn = rd.next::<IoPtn>()?;

        if ptn.rrr > 1 || ptn.rrr < 0 {
            return Err(ProjectReadError::FmtUnknown);
        }

        let mut noise_data = NoiseData::new();
        noise_data.read(rd)?;
        let vu = VoiceUnit {
            data: VoiceData::Noise(noise_data),
            flags: ptn.voice_flags,
            basic_key: i32::from(ptn.basic_key),
            tuning: ptn.tuning,
            ..Default::default()
        };
        Ok(Self::from_unit(vu))
    }

    pub(crate) fn write_mate_ptn(&self, out: &mut Vec<u8>, data: &NoiseData) {
        out.extend_from_slice(Tag::MatePTN.to_code());
        let io_size_pos = out.len();
        // Placeholder for io size
        out.extend_from_slice(&[0; 4]);
        let vu = &self.slots[0].unit;
        let ptn = IoPtn {
            x3x_unit_no: 0,
            basic_key: vu.basic_key.try_into().unwrap(),
            voice_flags: vu.flags,
            tuning: vu.tuning,
            rrr: 1,
        };
        out.extend_from_slice(bytemuck::bytes_of(&ptn));
        data.write(out);
        // Write io size retroactively
        let bytes_written: u32 = (out.len() - (io_size_pos + 4)).try_into().unwrap();
        out[io_size_pos..io_size_pos + 4].copy_from_slice(&bytes_written.to_le_bytes());
    }

    pub(crate) fn read_mate_ptv(rd: &mut crate::io::Reader) -> ReadResult<Self> {
        let _size: u32 = rd.next()?;
        let _ptv: IoPtv = rd.next()?;
        Self::ptv_read(rd)
    }
    pub(crate) fn write_mate_ptv(&self, out: &mut Vec<u8>) -> WriteResult {
        out.extend_from_slice(Tag::MatePTV.to_code());
        let io_ptv = IoPtv {
            x3x_unit_no: 0,
            rrr: 0,
            x3x_tuning: 0.0,
            size: 0,
        };
        let size: u32 = 0;
        let idx_before_written = out.len();
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(bytemuck::bytes_of(&io_ptv));
        self.ptv_write(out)?;
        let idx_after_written = out.len();
        #[expect(clippy::cast_possible_truncation)]
        let written_size = (idx_after_written - (idx_before_written + 4)) as u32;
        out[idx_before_written..idx_before_written + 4]
            .copy_from_slice(&written_size.to_le_bytes());
        let io_ptv_size_offset = idx_before_written + 12;
        let io_ptv_written_size = written_size - 12;
        out[io_ptv_size_offset..io_ptv_size_offset + 4]
            .copy_from_slice(&io_ptv_written_size.to_le_bytes());
        Ok(())
    }
    pub(crate) fn write_mate_oggv(&self, out: &mut Vec<u8>, data: &OggVData) {
        out.extend_from_slice(Tag::MateOGGV.to_code());
        let misc_size: u32 = 4 * 4; // ch, sps2, smp_num, size2
        #[expect(clippy::cast_possible_truncation)]
        let size: u32 = size_of::<IoOggv>() as u32 + data.raw_bytes.len() as u32 + misc_size;
        out.extend_from_slice(&size.to_le_bytes());
        let io_oggv: IoOggv = IoOggv {
            xxx: 0,
            basic_key: self.slots[0].unit.basic_key.try_into().unwrap(),
            voice_flags: self.slots[0].unit.flags,
            tuning: self.slots[0].unit.tuning,
        };
        out.extend_from_slice(bytemuck::bytes_of(&io_oggv));
        let ch: i32 = data.ch;
        out.extend_from_slice(&ch.to_le_bytes());
        let sps2: i32 = data.sps2;
        out.extend_from_slice(&sps2.to_le_bytes());
        let smp_num: i32 = data.smp_num;
        out.extend_from_slice(&smp_num.to_le_bytes());
        #[expect(clippy::cast_possible_truncation)]
        let size2: u32 = data.raw_bytes.len() as u32;
        out.extend_from_slice(&size2.to_le_bytes());
        if size2 == 0 {
            return;
        }
        out.extend_from_slice(&data.raw_bytes);
    }
    /// Read a voice from `.ptvoice` data
    pub fn from_ptvoice(data: &[u8]) -> ReadResult<Self> {
        let mut reader = crate::io::Reader { data, cur: 0 };
        Self::ptv_read(&mut reader)
    }
    /// Serialize to `.ptvoice` data
    pub fn to_ptvoice(&self) -> WriteResult<Vec<u8>> {
        let mut out = Vec::new();
        self.ptv_write(&mut out)?;
        Ok(out)
    }
    #[expect(clippy::inconsistent_digit_grouping)]
    fn ptv_read(rd: &mut crate::io::Reader) -> ReadResult<Self> {
        if &rd.next::<[u8; 8]>()? != b"PTVOICE-" {
            return Err(ProjectReadError::InvalidTag);
        }
        let ver: u32 = rd.next()?;

        if ver > 2006_01_11 {
            return Err(ProjectReadError::FmtNewer);
        }
        let _total: i32 = rd.next()?;
        let _x3x_basic = rd.next_varint()?;
        let work1 = rd.next_varint()?;
        let work2 = rd.next_varint()?;
        if work1 != 0 || work2 != 0 {
            return Err(ProjectReadError::FmtUnknown);
        }
        let num = rd.next_varint()?;
        match num {
            1 | 2 => {}
            _ => return Err(ProjectReadError::FmtUnknown),
        }
        let mut slots = ArrayVec::new();
        for _ in 0..num {
            let mut vu = VoiceUnit {
                basic_key: rd.next_varint()?.cast_signed(),
                volume: rd.next_varint()?.cast_signed().try_into().unwrap(),
                pan: rd.next_varint()?.cast_signed().try_into().unwrap(),
                tuning: f32::from_bits(rd.next_varint()?),
                flags: VoiceFlags::from_bits_retain(rd.next_varint()?),
                ..Default::default()
            };
            let data_flags = rd.next_varint()?;
            if data_flags & PTV_DATAFLAG_WAVE != 0 {
                let mut wave_data = WaveData::Coord {
                    points: Vec::new(),
                    resolution: 0,
                };
                read_wave(rd, &mut wave_data)?;
                vu.data = VoiceData::Wave(wave_data);
            }
            if data_flags & PTV_DATAFLAG_ENVELOPE != 0 {
                read_envelope(rd, &mut vu.envelope)?;
            }
            slots.push(VoiceSlot::from_unit(vu));
        }
        Ok(Self {
            slots,
            name: "<no name>".into(),
        })
    }
    #[expect(clippy::inconsistent_digit_grouping)]
    fn ptv_write(&self, out: &mut Vec<u8>) -> WriteResult {
        out.extend_from_slice(b"PTVOICE-");
        let ver: u32 = 2006_01_11;
        out.extend_from_slice(&ver.to_le_bytes());
        let total_offset = out.len();
        let total: i32 = 0;
        out.extend_from_slice(&total.to_le_bytes());
        let x3x_basic: u32 = 0;
        write_varint(x3x_basic, out);
        let work1: u32 = 0;
        let work2: u32 = 0;
        write_varint(work1, out);
        write_varint(work2, out);
        #[expect(clippy::cast_possible_truncation)]
        let ch_num: u32 = self.slots.len() as u32;
        write_varint(ch_num, out);
        for VoiceSlot { unit, .. } in &self.slots {
            write_varint(unit.basic_key.cast_unsigned(), out);
            write_varint(unit.volume.cast_unsigned().into(), out);
            write_varint(unit.pan.cast_unsigned().into(), out);
            write_varint(unit.tuning.to_bits(), out);
            write_varint(unit.flags.bits(), out);
            let mut data_flags = PTV_DATAFLAG_WAVE;
            if !unit.envelope.points.is_empty() {
                data_flags |= PTV_DATAFLAG_ENVELOPE;
            }
            write_varint(data_flags, out);
            let VoiceData::Wave(wave_data) = &unit.data else {
                unreachable!()
            };
            write_wave(wave_data, out)?;
            if !unit.envelope.points.is_empty() {
                write_envelope(&unit.envelope, out);
            }
        }
        let current_offset = out.len();
        #[expect(clippy::cast_possible_truncation)]
        let amount_written = (current_offset - (total_offset + 4)) as u32;
        out[total_offset..total_offset + 4].copy_from_slice(&amount_written.to_le_bytes());
        Ok(())
    }
    pub(crate) fn read_ogg(rd: &mut crate::io::Reader<'_>) -> ReadResult<Self> {
        let _size: u32 = rd.next()?;
        #[cfg_attr(not(feature = "oggv"), expect(unused_variables))]
        let io_oggv: IoOggv = rd.next()?;
        let ch: i32 = rd.next()?;
        let sps2: i32 = rd.next()?;
        let smp_num: i32 = rd.next()?;
        let size: u32 = rd.next()?;
        if size == 0 {
            return Err(ProjectReadError::InvalidData);
        }
        #[cfg(feature = "oggv")]
        {
            let unit = oggv::read(rd, &io_oggv, size as usize, ch, sps2, smp_num);
            Ok(Self::from_unit(unit))
        }
        #[cfg(not(feature = "oggv"))]
        {
            Err(ProjectReadError::OggvSupportDisabled)
        }
    }
}

#[derive(Clone, Copy, bytemuck::AnyBitPattern, bytemuck::NoUninit)]
#[repr(C)]
struct IoOggv {
    xxx: u16,
    basic_key: u16,
    voice_flags: VoiceFlags,
    tuning: f32,
}

fn read_wave(rd: &mut crate::io::Reader, wave_data: &mut WaveData) -> ReadResult {
    let kind = rd.next_varint()?;
    *wave_data = match kind {
        0 => {
            let num = rd.next_varint()?;
            let reso = rd.next_varint()?;
            let mut points = vec![OsciPt::ZERO; num as usize];
            for pt in &mut points {
                pt.x = u16::from(rd.next::<u8>()?);
                pt.y = i16::from(rd.next::<i8>()?);
            }
            WaveData::Coord {
                resolution: reso.try_into().unwrap(),
                points,
            }
        }
        1 => {
            let num = rd.next_varint()?;
            let mut points = vec![OsciPt::ZERO; num as usize];

            for pt in &mut points {
                let x = rd.next_varint()?;
                pt.x = x.try_into().map_err(|_| ProjectReadError::OvertonePointOutOfRange(x))?;
                let y = rd.next_varint()?;
                pt.y = i16::try_from(y.cast_signed())
                    .map_err(|_| ProjectReadError::OvertonePointOutOfRange(y))?;
            }
            WaveData::Overtone { points }
        }
        _ => panic!("Invalid/unsupported type: {kind}"),
    };

    Ok(())
}

fn write_wave(wave_data: &WaveData, out: &mut Vec<u8>) -> WriteResult {
    match wave_data {
        WaveData::Coord {
            resolution,
            points: coordinates,
        } => {
            write_varint(0, out);
            #[expect(clippy::cast_possible_truncation)]
            let num_pts: u32 = coordinates.len() as u32;
            write_varint(num_pts, out);
            write_varint(u32::from(*resolution), out);
            for pt in coordinates {
                // We use a 16 bit point type here because that's what oscillator
                // expects, but the PxTone format only saves 8 bits.
                let x = pt.x.try_into().map_err(|_| ProjectWriteError::CoordWavePointOutOfRange)?;
                let y: i8 =
                    pt.y.try_into().map_err(|_| ProjectWriteError::CoordWavePointOutOfRange)?;
                out.push(x);
                out.push(y.cast_unsigned());
            }
        }
        WaveData::Overtone {
            points: coordinates,
        } => {
            write_varint(1, out);
            #[expect(clippy::cast_possible_truncation)]
            let num_pts: u32 = coordinates.len() as u32;
            write_varint(num_pts, out);
            for pt in coordinates {
                write_varint(pt.x.into(), out);
                write_varint(i32::from(pt.y).cast_unsigned(), out);
            }
        }
    }
    Ok(())
}

fn read_envelope(rd: &mut crate::io::Reader, envelope: &mut EnvelopeSrc) -> ReadResult {
    envelope.seconds_per_point = rd.next_varint()?;
    let envelope_head = rd.next_varint()? as usize;
    let body_num = rd.next_varint()? as usize;
    if body_num != 0 {
        return Err(ProjectReadError::FmtUnknown);
    }
    let tail = rd.next_varint()? as usize;
    if tail != 1 {
        return Err(ProjectReadError::FmtUnknown);
    }
    let num = envelope_head + body_num + tail;
    envelope.points = vec![EnvPt::ZERO; num];
    for pt in &mut envelope.points {
        pt.x = rd.next_varint()?.try_into().unwrap();
        pt.y = rd.next_varint()?.try_into().unwrap();
    }
    Ok(())
}

fn write_envelope(envelope: &EnvelopeSrc, out: &mut Vec<u8>) {
    write_varint(envelope.seconds_per_point, out);
    let envelope_head = envelope.points.len().saturating_sub(1);
    #[expect(clippy::cast_possible_truncation)]
    write_varint(envelope_head as u32, out);
    let tail = 1;
    #[expect(clippy::cast_possible_truncation)]
    let body_num = envelope.points.len() as u32 - (envelope_head as u32 + tail);
    assert_eq!(body_num, 0);
    write_varint(body_num, out);
    write_varint(tail, out);
    for pt in &envelope.points {
        write_varint(pt.x.into(), out);
        write_varint(pt.y.into(), out);
    }
}

#[derive(Clone, Copy, bytemuck::AnyBitPattern, bytemuck::NoUninit)]
#[repr(C)]
struct IoPtv {
    x3x_unit_no: u16,
    rrr: u16,
    x3x_tuning: f32,
    size: i32,
}

const PTV_DATAFLAG_WAVE: u32 = 1;
const PTV_DATAFLAG_ENVELOPE: u32 = 2;
