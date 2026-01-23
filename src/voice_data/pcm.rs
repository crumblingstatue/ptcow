use crate::{Bps, ChNum, SampleRate, SourceSampleRate};

/// Describes PCM (Pulse Code Modulation) voice data
#[derive(Clone, Default)]
pub struct PcmData {
    /// Number of channels (mono or stereo)
    pub ch: ChNum,
    /// Sample rate of the PCM sound
    pub sps: SourceSampleRate,
    /// Bits per sample (8 or 16)
    pub bps: Bps,
    /// Number of samples
    ///
    /// Not the same as the length of the 8 bit sample buffer, since the PCM data might be 16 bit.
    /// Also don't forget stereo.
    pub num_samples: u32,
    /// 8 bit sample buffer containint the raw sample data
    pub smp: Vec<u8>,
}

impl PcmData {
    pub(crate) fn create(&mut self, ch: ChNum, sps: SourceSampleRate, bps: Bps, sample_num: u32) {
        self.ch = ch;
        self.sps = sps;
        self.bps = bps;
        self.num_samples = sample_num;

        let size: usize = self.num_samples as usize * self.bps as usize * self.ch as usize / 8;

        self.smp = match self.bps {
            Bps::B8 => vec![128; size],
            Bps::B16 => vec![0; size],
        };
    }

    pub(crate) fn to_converted(&self, new_samp_rate: SampleRate) -> (u32, Vec<u8>) {
        let mut new = self.clone();
        new.convert_to_bps_16();
        new.convert_to_stereo();
        new.into_converted_sps(new_samp_rate)
    }

    pub(crate) fn into_sample_buf(self) -> Vec<u8> {
        self.smp
    }

    pub(crate) fn sample_mut(&mut self) -> &mut [u8] {
        &mut self.smp
    }

    pub(crate) fn new() -> Self {
        Self::default()
    }
    fn convert_to_stereo(&mut self) {
        let sample_size: usize =
            (self.num_samples as usize) * self.ch as usize * self.bps as usize / 8;

        if self.ch == ChNum::Stereo {
            return;
        }

        let work_size = sample_size * 2;
        let mut buf = vec![0; work_size];

        match self.bps {
            Bps::B8 => {
                let mut b = 0;
                let mut a = 0;
                while a < sample_size {
                    buf[b] = self.smp[a];
                    buf[b + 1] = self.smp[a];
                    b += 2;
                    a += 1;
                }
            }
            Bps::B16 => {
                let mut b = 0;
                let mut a = 0;
                while a < sample_size {
                    buf[b] = self.smp[a];
                    buf[b + 1] = self.smp[a + 1];
                    buf[b + 2] = self.smp[a];
                    buf[b + 3] = self.smp[a + 1];
                    b += 4;
                    a += 2;
                }
            }
        }
        self.smp.resize(work_size, 0);
        self.smp.copy_from_slice(&buf[..work_size]);

        self.ch = ChNum::Stereo;
    }
    fn convert_to_bps_16(&mut self) {
        if self.bps == Bps::B16 {
            return;
        }

        let sample_size: u32 = self.num_samples * self.ch as u32 * self.bps as u32 / 8;

        let work_size = sample_size * 2;
        let mut work_buf = vec![0; work_size as usize];
        let mut b: usize = 0;
        let mut a: usize = 0;
        while a < sample_size as usize {
            let mut temp1 = i16::from(self.smp[a]);
            temp1 = (temp1 - 128) * 0x100;
            let w_buf_i16: &mut i16 = bytemuck::from_bytes_mut(&mut work_buf[b..b + 2]);
            *w_buf_i16 = temp1;
            b += 2;
            a += 1;
        }
        self.smp.resize(work_size as usize, 0);
        self.smp.copy_from_slice(&work_buf[..work_size as usize]);
        self.bps = Bps::B16;
    }

    fn into_converted_sps(self, new_sps: SampleRate) -> (u32, Vec<u8>) {
        // This function should only be called after channel num and sample rate conversion
        assert!(self.ch == ChNum::Stereo && self.bps == Bps::B16);
        if self.sps == new_sps.into() {
            return (self.num_samples, self.smp);
        }

        let mut head_size = self.ch as u32 * self.bps as u32 / 8;
        let mut body_size = self.num_samples * self.ch as u32 * self.bps as u32 / 8;
        let mut tail_size = self.ch as u32 * self.bps as u32 / 8;

        head_size = (u64::from(head_size) * u64::from(new_sps))
            .div_ceil(u64::from(self.sps))
            .try_into()
            .unwrap();
        body_size = (u64::from(body_size) * u64::from(new_sps))
            .div_ceil(u64::from(self.sps))
            .try_into()
            .unwrap();
        tail_size = (u64::from(tail_size) * u64::from(new_sps))
            .div_ceil(u64::from(self.sps))
            .try_into()
            .unwrap();

        let mut work_size = head_size + body_size + tail_size;

        let sample_num = work_size / 4;
        work_size = sample_num * 4;
        let as_u32 = bytemuck::pod_collect_to_vec::<_, u32>(&self.smp);
        let mut u32_buf: Vec<u32> = vec![0; work_size as usize];
        for (i, u32_samp) in u32_buf.iter_mut().take(sample_num as usize).enumerate() {
            let idx = i * self.sps as usize / usize::from(new_sps);
            if let Some(samp) = as_u32.get(idx) {
                *u32_samp = *samp;
            } else {
                eprintln!("into_converted_sps: Out of bounds ({idx})");
                break;
            }
        }
        (
            body_size / 4,
            bytemuck::cast_slice(&u32_buf)[..work_size as usize].to_vec(),
        )
    }
}
