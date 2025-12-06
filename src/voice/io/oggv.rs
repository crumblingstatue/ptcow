use {
    super::IoOggv,
    crate::{VoiceData, voice_data::oggv::OggVData},
};

pub fn read(
    rd: &mut crate::io::Reader,
    io_oggv: &IoOggv,
    size: usize,
    unit: &mut crate::voice::VoiceUnit,
    ch: i32,
    sps2: i32,
    smp_num: i32,
) {
    unit.data = VoiceData::OggV(OggVData {
        raw_bytes: rd.data[rd.cur..rd.cur + size].to_vec(),
        ch,
        sps2,
        smp_num,
    });
    rd.cur += size;
    unit.flags = io_oggv.voice_flags;
    unit.basic_key = i32::from(io_oggv.basic_key);
    unit.tuning = io_oggv.tuning;
}
