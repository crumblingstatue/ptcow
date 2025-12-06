use {
    super::IoOggv,
    crate::{Bps, ChNum, PcmData, ProjectReadError, ReadResult, VoiceData},
    bytemuck::Contiguous,
};

pub fn read(
    rd: &mut crate::io::Reader,
    io_oggv: &IoOggv,
    size: usize,
    unit: &mut crate::voice::VoiceUnit,
) -> ReadResult {
    let mut dec = vorbis_rs::VorbisDecoder::<&[u8]>::new(&rd.data[rd.cur..rd.cur + size])
        .map_err(|_| ProjectReadError::OggvReadError)?;
    let mut pcm = PcmData::new();
    pcm.sps = dec.sampling_frequency().into_integer();
    pcm.ch = match dec.channels().into_integer() {
        1 => ChNum::Mono,
        2 => ChNum::Stereo,
        _ => panic!("Vorbis channel number >2 not supported."),
    };
    pcm.bps = Bps::B16;
    let mut i16_samples: Vec<i16> = Vec::new();
    while let Some(block) = dec.decode_audio_block().map_err(|_| ProjectReadError::OggvReadError)? {
        let interleaved = planar_to_interleaved(block.samples());
        for sample in interleaved {
            #[expect(clippy::cast_possible_truncation)]
            i16_samples.push((sample * 32768.0).round_ties_even() as i16);
        }
    }
    pcm.smp = bytemuck::pod_collect_to_vec(&i16_samples);
    #[expect(clippy::cast_possible_truncation)]
    (pcm.num_samples = pcm.smp.len() as u32 / 2);
    rd.cur += size;
    unit.data = VoiceData::Pcm(pcm);
    unit.flags = io_oggv.voice_flags;
    unit.basic_key = i32::from(io_oggv.basic_key);
    unit.tuning = io_oggv.tuning;
    Ok(())
}

fn planar_to_interleaved(planar: &[&[f32]]) -> Vec<f32> {
    let channels = planar.len();
    let frames = planar[0].len();

    let mut out = Vec::with_capacity(channels * frames);

    for i in 0..frames {
        for ch in planar {
            out.push(ch[i]);
        }
    }

    out
}
