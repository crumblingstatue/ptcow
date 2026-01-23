use crate::{Bps, ChNum, PcmData};

/// Ogg/Vorbis voice data
#[derive(Clone)]
pub struct OggVData {
    /// Raw Ogg/Vorbis data
    pub raw_bytes: Vec<u8>,
    /// Channel number
    pub ch: i32,
    /// Samples per second
    pub sps2: i32,
    /// Number of samples
    pub smp_num: i32,
}

#[cfg(feature = "oggv")]
pub fn decode_oggv(raw_data: &[u8]) -> Option<PcmData> {
    use symphonia_core::{codecs::Decoder, formats::FormatReader as _};
    let media_stream = symphonia_core::io::MediaSourceStream::new(
        Box::new(std::io::Cursor::new(raw_data.to_vec())),
        symphonia_core::io::MediaSourceStreamOptions::default(),
    );
    let mut ogg_reader = symphonia_format_ogg::OggReader::try_new(
        media_stream,
        &symphonia_core::formats::FormatOptions::default(),
    )
    .unwrap();
    let track = ogg_reader.default_track()?;

    let mut pcm = PcmData::new();
    pcm.sps = track.codec_params.sample_rate?;
    pcm.ch = match track.codec_params.channels?.count() {
        1 => ChNum::Mono,
        2 => ChNum::Stereo,
        _ => panic!("Vorbis channel number >2 not supported."),
    };
    pcm.bps = Bps::B16;
    let mut i16_samples: Vec<i16> = Vec::new();
    let mut vorbis_decoder = symphonia_codec_vorbis::VorbisDecoder::try_new(
        &track.codec_params,
        &symphonia_core::codecs::DecoderOptions { verify: true },
    )
    .unwrap();
    let delay = track.codec_params.delay;
    let padding = track.codec_params.padding;
    while let Ok(packet) = ogg_reader.next_packet() {
        use symphonia_core::audio::AudioBufferRef;
        let buf_ref = vorbis_decoder.decode(&packet).unwrap();
        let AudioBufferRef::F32(buf) = buf_ref else {
            panic!("Expected f32 Ogg/Vorbis samples");
        };
        let interleaved = planar_to_interleaved(buf.planes().planes());
        for sample in interleaved {
            #[expect(clippy::cast_possible_truncation)]
            i16_samples.push((sample * 32768.0).round_ties_even() as i16);
        }
    }
    if let Some(delay) = delay {
        i16_samples.truncate(i16_samples.len().saturating_sub(delay as usize));
    }
    if let Some(padding) = padding {
        i16_samples.truncate(i16_samples.len().saturating_sub(padding as usize));
    }

    pcm.smp = bytemuck::pod_collect_to_vec(&i16_samples);
    #[expect(clippy::cast_possible_truncation)]
    (pcm.num_samples = pcm.smp.len() as u32 / 2 / pcm.ch as u32);
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
