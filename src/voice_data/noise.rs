use arrayvec::ArrayVec;

use crate::{
    EnvPt, NATIVE_SAMPLE_RATE,
    io::write_varint,
    noise_builder::{NoiseDesignOscillator, NoiseType},
    result::{ProjectReadError, ReadResult},
};

/// Noise generated with a waveform generator
#[derive(Default, Clone)]
pub struct NoiseData {
    /// Number of samples for 44 khz sample rate
    pub smp_num_44k: u32,
    /// Noise design units that are mixed together to generate the final waveform
    pub units: ArrayVec<NoiseDesignUnit, 4>,
    /// Only used for serialization
    pub(crate) io_size: u32,
}

#[expect(clippy::inconsistent_digit_grouping)]
const LATEST_VER: u32 = 2012_04_18;

const NOISE_TAG: &[u8; 8] = b"PTNOISE-";

impl NoiseData {
    pub(crate) fn read(&mut self, rd: &mut crate::io::Reader) -> ReadResult {
        let mut design_unit: &mut NoiseDesignUnit;

        self.release();
        if rd.next::<[u8; 8]>()? != *NOISE_TAG {
            return Err(ProjectReadError::InvalidTag);
        }
        let ver = rd.next::<u32>()?;
        if ver > LATEST_VER {
            return Err(ProjectReadError::FmtNewer);
        }
        self.smp_num_44k = rd.next_varint()?;
        let unit_num = rd.next::<u8>()?;
        #[expect(clippy::cast_possible_truncation)]
        if unit_num > self.units.capacity() as u8 {
            return Err(ProjectReadError::FmtUnknown);
        }

        for _ in 0..unit_num {
            self.units.push(NoiseDesignUnit::default());
        }
        for u in 0..unit_num {
            design_unit = &mut self.units[u as usize];
            let flags = rd.next_varint()?;
            design_unit.io_flags = flags;

            if flags & NOISEEDITFLAG_UNCOVERED != 0 {
                return Err(ProjectReadError::FmtUnknown);
            }

            if flags & NOISEEDITFLAG_ENVELOPE != 0 {
                let enve_num = rd.next_varint()? as usize;
                if enve_num > MAX_NOISEEDITENVELOPENUM {
                    return Err(ProjectReadError::FmtUnknown);
                }
                design_unit.enves.clear();
                for _ in 0..enve_num {
                    design_unit.enves.push(EnvPt {
                        x: rd.next_varint()?.try_into().unwrap(),
                        y: rd.next_varint()?.try_into().unwrap(),
                    });
                }
            }
            if flags & NOISEEDITFLAG_PAN != 0 {
                design_unit.pan = rd.next::<i8>()?;
            }

            if flags & NOISEEDITFLAG_OSC_MAIN != 0 {
                read_oscillator(&mut design_unit.main, rd)?;
            }
            if flags & NOISEEDITFLAG_OSC_FREQ != 0 {
                read_oscillator(&mut design_unit.freq, rd)?;
            }
            if flags & NOISEEDITFLAG_OSC_VOLU != 0 {
                read_oscillator(&mut design_unit.volu, rd)?;
            }
        }

        Ok(())
    }

    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(NOISE_TAG);
        // TODO: Not always true(?)
        let ver = LATEST_VER;
        out.extend_from_slice(&ver.to_le_bytes());
        write_varint(self.smp_num_44k, out);
        let unit_num: u8 = self.units.len().try_into().unwrap();
        out.push(unit_num);
        for unit in &self.units {
            write_varint(unit.io_flags, out);
            if unit.io_flags & NOISEEDITFLAG_ENVELOPE != 0 {
                let enve_num: u32 = unit.enves.len().try_into().unwrap();
                write_varint(enve_num, out);
                for pt in &unit.enves {
                    write_varint(pt.x.into(), out);
                    write_varint(pt.y.into(), out);
                }
            }
            if unit.io_flags & NOISEEDITFLAG_PAN != 0 {
                out.push(unit.pan.cast_unsigned());
            }
            if unit.io_flags & NOISEEDITFLAG_OSC_MAIN != 0 {
                write_oscillator(&unit.main, out);
            }
            if unit.io_flags & NOISEEDITFLAG_OSC_FREQ != 0 {
                write_oscillator(&unit.freq, out);
            }
            if unit.io_flags & NOISEEDITFLAG_OSC_VOLU != 0 {
                write_oscillator(&unit.volu, out);
            }
        }
    }

    pub(crate) fn release(&mut self) {
        self.units.clear();
    }

    pub(crate) fn fix(&mut self) {
        if self.smp_num_44k > NOISEDESIGNLIMIT_SMPNUM {
            self.smp_num_44k = NOISEDESIGNLIMIT_SMPNUM;
        }

        for design_unit in &mut self.units {
            for enve in &mut design_unit.enves {
                enve.x = enve.x.clamp(0, NOISEDESIGNLIMIT_ENVE_X);
                enve.y = enve.y.clamp(0, NOISEDESIGNLIMIT_ENVE_Y);
            }
            design_unit.pan = design_unit.pan.clamp(-100, 100);
            fix_unit(&mut design_unit.main);
            fix_unit(&mut design_unit.freq);
            fix_unit(&mut design_unit.volu);
        }
    }

    pub(crate) const fn get_unit_num(&self) -> usize {
        self.units.len()
    }

    pub(crate) fn new() -> Self {
        Self::default()
    }
}

const MAX_NOISEEDITENVELOPENUM: usize = 3;
//const NOISEEDITFLAG_XX1: u32 = 0x0001;
//const NOISEEDITFLAG_XX2: u32 = 0x0002;
const NOISEEDITFLAG_ENVELOPE: u32 = 0x0004;
const NOISEEDITFLAG_PAN: u32 = 0x0008;
const NOISEEDITFLAG_OSC_MAIN: u32 = 0x0010;
const NOISEEDITFLAG_OSC_FREQ: u32 = 0x0020;
const NOISEEDITFLAG_OSC_VOLU: u32 = 0x0040;
//const NOISEEDITFLAG_OSC_PAN: u32 = 0x0080;
const NOISEEDITFLAG_UNCOVERED: u32 = 0xffff_ff83;

const NOISEDESIGNLIMIT_SMPNUM: u32 = 48000 * 10;
const NOISEDESIGNLIMIT_ENVE_X: u16 = 1000 * 10;
const NOISEDESIGNLIMIT_ENVE_Y: u8 = 100;
const NOISEDESIGNLIMIT_OSC_FREQUENCY: f32 = NATIVE_SAMPLE_RATE as f32;
const NOISEDESIGNLIMIT_OSC_VOLUME: f32 = 200.0;
const NOISEDESIGNLIMIT_OSC_OFFSET: f32 = 100.0;

const fn fix_unit(osc: &mut NoiseDesignOscillator) {
    osc.freq = osc.freq.clamp(0., NOISEDESIGNLIMIT_OSC_FREQUENCY);
    osc.volume = osc.volume.clamp(0., NOISEDESIGNLIMIT_OSC_VOLUME);
    osc.offset = osc.offset.clamp(0., NOISEDESIGNLIMIT_OSC_OFFSET);
}

#[expect(clippy::cast_precision_loss)]
fn read_oscillator(osc: &mut NoiseDesignOscillator, rd: &mut crate::io::Reader) -> ReadResult {
    let wave_type = rd.next_varint()?;

    let type_ = match wave_type {
        0 => panic!("None wave type detected"),
        1 => NoiseType::Sine,
        2 => NoiseType::Saw,
        3 => NoiseType::Rect,
        4 => NoiseType::Random,
        5 => NoiseType::Saw2,
        6 => NoiseType::Rect2,
        7 => NoiseType::Tri,
        8 => NoiseType::Random2,
        9 => NoiseType::Rect3,
        10 => NoiseType::Rect4,
        11 => NoiseType::Rect8,
        12 => NoiseType::Rect16,
        13 => NoiseType::Saw3,
        14 => NoiseType::Saw4,
        15 => NoiseType::Saw6,
        16 => NoiseType::Saw8,
        _ => return Err(ProjectReadError::FmtUnknown),
    };
    osc.type_ = type_;
    osc.invert = rd.next_varint()? != 0;
    osc.freq = rd.next_varint()? as f32 / 10.;
    osc.volume = rd.next_varint()? as f32 / 10.;
    osc.offset = rd.next_varint()? as f32 / 10.;

    Ok(())
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn write_oscillator(osc: &NoiseDesignOscillator, out: &mut Vec<u8>) {
    let type_id: u32 = match osc.type_ {
        NoiseType::Sine => 1,
        NoiseType::Saw => 2,
        NoiseType::Rect => 3,
        NoiseType::Random => 4,
        NoiseType::Saw2 => 5,
        NoiseType::Rect2 => 6,
        NoiseType::Tri => 7,
        NoiseType::Random2 => 8,
        NoiseType::Rect3 => 9,
        NoiseType::Rect4 => 10,
        NoiseType::Rect8 => 11,
        NoiseType::Rect16 => 12,
        NoiseType::Saw3 => 13,
        NoiseType::Saw4 => 14,
        NoiseType::Saw6 => 15,
        NoiseType::Saw8 => 16,
    };
    write_varint(type_id, out);
    write_varint(u32::from(osc.invert), out);
    write_varint((osc.freq * 10.) as u32, out);
    write_varint((osc.volume * 10.) as u32, out);
    write_varint((osc.offset * 10.) as u32, out);
}

/// Describes how to generate a noise design waveform
#[derive(Clone, Default)]
pub struct NoiseDesignUnit {
    /// Envelope points
    pub enves: ArrayVec<EnvPt, 3>,
    /// Panning
    pub pan: i8,
    /// Main (base) oscillator
    pub main: NoiseDesignOscillator,
    /// Frequency oscillator
    pub freq: NoiseDesignOscillator,
    /// Volume oscillator
    pub volu: NoiseDesignOscillator,
    /// Currently only used for serialization
    /// TODO: Possibly can be generated instead
    pub(crate) io_flags: u32,
}
