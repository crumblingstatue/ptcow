use {
    crate::{Bps, ChNum, PcmData},
    bytemuck::Contiguous as _,
};

#[derive(Clone)]
pub struct OggVData {
    pub raw_bytes: Vec<u8>,
    pub ch: i32,
    pub sps2: i32,
    pub smp_num: i32,
    pub size: u32,
}

pub fn decode_oggv(raw_data: &[u8]) -> Option<PcmData> {
    let mut dec = vorbis_rs::VorbisDecoder::<&[u8]>::new(raw_data).ok()?;
    let mut pcm = PcmData::new();
    pcm.sps = dec.sampling_frequency().into_integer();
    pcm.ch = match dec.channels().into_integer() {
        1 => ChNum::Mono,
        2 => ChNum::Stereo,
        _ => panic!("Vorbis channel number >2 not supported."),
    };
    pcm.bps = Bps::B16;
    let mut i16_samples: Vec<i16> = Vec::new();
    while let Some(block) = dec.decode_audio_block().ok()? {
        let interleaved = planar_to_interleaved(block.samples());
        for sample in interleaved {
            #[expect(clippy::cast_possible_truncation)]
            i16_samples.push((sample * 32768.0).round_ties_even() as i16);
        }
    }
    pcm.smp = bytemuck::pod_collect_to_vec(&i16_samples);
    #[expect(clippy::cast_possible_truncation)]
    (pcm.num_samples = pcm.smp.len() as u32 / 2);
    Some(pcm)
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
