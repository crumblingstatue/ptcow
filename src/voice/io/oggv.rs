use {
    super::IoOggv,
    crate::{VoiceData, VoiceUnit, voice_data::oggv::OggVData},
};

pub fn read(
    rd: &mut crate::io::Reader,
    io_oggv: &IoOggv,
    size: usize,
    ch: i32,
    sps2: i32,
    smp_num: i32,
) -> VoiceUnit {
    let data = VoiceData::OggV(OggVData {
        raw_bytes: rd.data[rd.cur..rd.cur + size].to_vec(),
        ch,
        sps2,
        smp_num,
    });
    rd.cur += size;
    VoiceUnit {
        data,
        flags: io_oggv.voice_flags,
        basic_key: i32::from(io_oggv.basic_key),
        tuning: io_oggv.tuning,
        ..VoiceUnit::defaults()
    }
}
