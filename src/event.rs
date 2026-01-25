use crate::{
    io::write_varint,
    result::{ProjectReadError, ReadResult},
    timing::Tick,
    unit::{GroupIdx, UnitIdx, VoiceIdx},
};

/// List of [`Event`]s.
///
/// INVARIANT: ptcow's playback code assumes that events are sorted
/// by tick value in ascending order.
/// Use [`Self::sort`] after you made modifications to the event list,
/// to ensure correct playback.
#[derive(Default)]
pub struct EveList {
    /// The inner list of events
    pub eves: Vec<Event>,
    /// Unknown use, only used for serialization purposes. We just write back
    /// this value when we serialize. It being a "size" seems to be a lie.
    /// Or at least it is in newer format versions.
    ser_size: u32,
}

impl EveList {
    pub(crate) fn get_max_tick(&self) -> Tick {
        let mut max_clock: Tick = 0;
        let mut clock: Tick;

        for eve in &self.eves {
            if let Some(clock_val) = event_duration(eve.payload) {
                clock = eve.tick + clock_val;
            } else {
                clock = eve.tick;
            }
            if clock > max_clock {
                max_clock = clock;
            }
        }

        max_clock
    }

    pub(crate) fn read(rd: &mut crate::io::Reader) -> ReadResult<Self> {
        let size = rd.next::<u32>()?;
        let eve_num = rd.next::<u32>()?;

        let mut absolute: u32 = 0;
        let mut eves = Vec::new();

        for _ in 0..eve_num {
            let clock = rd.next_varint()?;
            let unit_no = UnitIdx(rd.next::<u8>()?);
            let kind = rd.next::<u8>()?;
            let value = rd.next_varint()?;
            let payload = match kind {
                0 => EventPayload::Null,
                1 => EventPayload::On { duration: value },
                2 => EventPayload::Key(value.try_into().unwrap()),
                3 => EventPayload::PanVol(value.try_into().unwrap()),
                4 => EventPayload::Velocity(value.cast_signed().try_into().unwrap()),
                5 => EventPayload::Volume(value.cast_signed().try_into().unwrap()),
                6 => EventPayload::Portament { duration: value },
                7 => EventPayload::BeatClock,
                8 => EventPayload::BeatTempo,
                9 => EventPayload::BeatNum,
                10 => EventPayload::Repeat,
                11 => EventPayload::Last,
                12 => EventPayload::SetVoice(VoiceIdx(value.try_into().unwrap())),
                13 => EventPayload::SetGroup(GroupIdx(value.try_into().unwrap())),
                14 => EventPayload::Tuning(bytemuck::cast(value)),
                15 => EventPayload::PanTime(value.try_into().unwrap()),
                _ => return Err(ProjectReadError::InvalidData),
            };
            absolute += clock;
            eves.push(Event {
                payload,
                unit: unit_no,
                tick: absolute,
            });
        }

        Ok(Self {
            eves,
            ser_size: size,
        })
    }

    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.ser_size.to_le_bytes());
        // Write dummy len
        let eve_num_offset = out.len();
        out.extend_from_slice(&[0u8; 4]);
        let mut eve_num: u32 = 0;
        let mut absolute: u32 = 0;
        for eve in &self.eves {
            if let EventPayload::PtcowDebug(_) = eve.payload {
                // We ignore debug events
                continue;
            }
            let clock = eve.tick - absolute;
            write_varint(clock, out);
            absolute = eve.tick;
            out.push(eve.unit.0);
            let (kind, value): (u8, u32) = match &eve.payload {
                EventPayload::Null => (0, 0),
                EventPayload::On { duration } => (1, *duration),
                EventPayload::Key(k) => (2, k.cast_unsigned()),
                EventPayload::PanVol(vol) => (3, u32::from(*vol)),
                EventPayload::Velocity(vel) => (4, vel.cast_unsigned().into()),
                EventPayload::Volume(vol) => (5, vol.cast_unsigned().into()),
                EventPayload::Portament { duration } => (6, *duration),
                EventPayload::BeatClock => (7, 0),
                EventPayload::BeatTempo => (8, 0),
                EventPayload::BeatNum => (9, 0),
                EventPayload::Repeat => (10, 0),
                EventPayload::Last => (11, 0),
                EventPayload::SetVoice(n) => (12, u32::from(n.0)),
                EventPayload::SetGroup(g) => (13, u32::from(g.0)),
                EventPayload::Tuning(t) => (14, t.to_bits()),
                EventPayload::PanTime(t) => (15, u32::from(*t)),
                EventPayload::PtcowDebug(_) => {
                    // We ignore debug events
                    continue;
                }
            };
            out.push(kind);
            write_varint(value, out);
            eve_num += 1;
        }
        out[eve_num_offset..eve_num_offset + 4].copy_from_slice(&eve_num.to_le_bytes());
    }
    /// Sort the events by their tick values, to ensure correct playback.
    pub fn sort(&mut self) {
        self.eves.sort_by_key(|eve| eve.tick);
    }
}

const fn event_duration(payload: EventPayload) -> Option<u32> {
    match payload {
        EventPayload::On { duration } | EventPayload::Portament { duration } => Some(duration),
        _ => None,
    }
}

pub const DEFAULT_VOLUME: u16 = 104;
pub const DEFAULT_VELOCITY: u16 = 104;
/// The default [`Key`] units start out with
pub const DEFAULT_KEY: Key = 24576;
pub const DEFAULT_BASICKEY: u32 = 17664;
pub const DEFAULT_TUNING: f32 = 1.0;

/// Payload of an event
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventPayload {
    /// Do nothing, and terminate playback if this event is encountered
    Null,
    /// Turn the target unit on for the specified number of ticks
    On {
        /// Number of ticks to turn the unit on for
        duration: Tick,
    },
    /// Change the key of the target unit
    Key(Key),
    /// Set the pan volume for the target unit.
    ///
    /// Normally should be in the range of `0..=128`, where 0 is left, and 128 is right.
    /// However, there are songs that go above 128. TODO: Document exact behavior when it's above 128.
    PanVol(u8),
    /// Set the [`velocity`](crate::Unit::velocity) attribute of the target unit
    Velocity(i16),
    /// Set the [`volume`](crate::Unit::volume) attribute of the target unit
    Volume(i16),
    /// Set up a slide from the current note to the next
    Portament {
        /// How long it takes for the slide to happen
        duration: Tick,
    },
    /// Ignored. Only present for compatibility reasons.
    BeatClock,
    /// Ignored. Only present for compatibility reasons.
    BeatTempo,
    /// Ignored. Only present for compatibility reasons.
    BeatNum,
    /// Ignored. Only present for compatibility reasons.
    Repeat,
    /// Ignored. Only present for compatibility reasons.
    Last,
    /// Set the voice index of the target unit
    SetVoice(VoiceIdx),
    /// Set the group index of the target unit
    SetGroup(GroupIdx),
    /// Set the [`tuning`](crate::Unit::tuning) property of the target unit
    Tuning(f32),
    /// Sets an effect where the left and right audio channels for the unit are sampled at different
    /// offsets.
    ///
    /// Range is within `0..64`.
    PanTime(u8),
    /// This event is ignored during playback, but you can insert it into the event stream for
    /// debugging purposes, because it can show in a GUI event viewer for example.
    PtcowDebug(i32),
}

impl EventPayload {
    /// Get the discriminant value of the event payload as `u8`
    #[must_use]
    pub const fn discriminant(&self) -> u8 {
        unsafe { *std::ptr::from_ref(self).cast() }
    }
}

// We probably don't want the event payload to get too big.
const _: () = assert!(size_of::<EventPayload>() == 8);

/// 1/256 of a semitone.
///
/// A semitone is the smallest distance between keys on a piano.
pub type Key = i32;

/// Song event
#[derive(Copy, Clone)]
pub struct Event {
    /// The payload of the event
    pub payload: EventPayload,
    /// The unit the event belongs to
    pub unit: UnitIdx,
    /// The clock tick the event place takes at
    pub tick: Tick,
}
