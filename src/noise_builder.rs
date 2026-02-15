use {
    crate::{
        Bps, ChNum, NATIVE_SAMPLE_RATE, SampleRate,
        pulse_frequency::PULSE_FREQ,
        pulse_oscillator::{OsciArgs, OsciPt, coord, overtone},
        voice_data::{
            noise::{NoiseData, NoiseDesignUnit},
            pcm::PcmData,
        },
    },
    std::{cmp::Ordering, iter::zip},
};

type Tables = [Box<[i16]>; 16];

/// Contains wave tables for generating different kinds of noises.
///
/// Used by [`noise_to_pcm`].
pub struct NoiseTable {
    pub(crate) inner: Tables,
}

struct Rng {
    buf: [i32; 2],
}

impl Default for Rng {
    fn default() -> Self {
        Self {
            buf: [0x4444, 0x8888],
        }
    }
}

impl Rng {
    #[expect(clippy::cast_possible_truncation)]
    fn next(&mut self) -> i16 {
        let mut w1 = self.buf[0] + self.buf[1];
        let mut w2: i32 = 0;
        let p1: &mut [i8; 4] = bytemuck::cast_mut(&mut w1);
        let p2: &mut [i8; 4] = bytemuck::cast_mut(&mut w2);
        p2[0] = p1[1];
        p2[1] = p1[0];
        self.buf[1] = self.buf[0];
        self.buf[0] = w2;

        w2 as i16
    }
}

const fn pt(x: u16, y: i16) -> OsciPt {
    OsciPt { x, y }
}

struct Chunker<'a, T> {
    head: usize,
    slice: &'a mut [T],
}

impl<'a, T> Chunker<'a, T> {
    const fn new(slice: &'a mut [T]) -> Self {
        Self { head: 0, slice }
    }
    fn next_until<const N: usize>(&mut self) -> &mut [T] {
        let chunk = self.slice[self.head..].first_chunk_mut::<N>().unwrap();
        self.head = N;
        chunk
    }
    fn rest(&mut self) -> &mut [T] {
        &mut self.slice[self.head..]
    }
}

impl Default for NoiseTable {
    fn default() -> Self {
        Self::generate()
    }
}

impl NoiseTable {
    /// Generate a new [`NoiseTable`].
    #[expect(clippy::cast_possible_truncation, reason = "f64 to i16 casts")]
    #[must_use]
    pub fn generate() -> Self {
        let overtones_sine = [pt(1, 128)];
        let overtones_saw2 = [
            pt(1, 128),
            pt(2, 128),
            pt(3, 128),
            pt(4, 128),
            pt(5, 128),
            pt(6, 128),
            pt(7, 128),
            pt(8, 128),
            pt(9, 128),
            pt(10, 128),
            pt(11, 128),
            pt(12, 128),
            pt(13, 128),
            pt(14, 128),
            pt(15, 128),
            pt(16, 128),
        ];
        let overtones_rect2 = [
            pt(1, 128),
            pt(3, 128),
            pt(5, 128),
            pt(7, 128),
            pt(9, 128),
            pt(11, 128),
            pt(13, 128),
            pt(15, 128),
        ];

        let coord_tri = [
            pt(0, 0),
            pt(SMP_NUM / 4, 128),
            pt(SMP_NUM * 3 / 4, -128),
            pt(SMP_NUM, 0),
        ];

        let tables = [
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            vec![0; 2 * SMP_NUM_RAND as usize].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
            [0; 2 * SMP_NUM_U].into(),
        ];

        let mut this = Self { inner: tables };

        let osci = OsciArgs {
            volume: 128,
            sample_num: SMP_NUM.into(),
        };
        for (s, p) in zip(0..SMP_NUM, &mut this.inner[NoiseType::Sine as usize]) {
            let ovt = overtone(osci, &overtones_sine, s).clamp(-1.0, 1.0);
            *p = (ovt * f64::from(SAMPLING_TOP)) as i16;
        }

        for (s, p) in zip(0..SMP_NUM, &mut this.inner[NoiseType::Saw as usize]) {
            let st2 = f64::from(SAMPLING_TOP) * 2.0;
            *p = (f64::from(SAMPLING_TOP) - st2 * f64::from(s) / f64::from(SMP_NUM)) as i16;
        }

        let mut s = 0;
        while s < SMP_NUM / 2 {
            this.inner[NoiseType::Rect as usize][s as usize] = SAMPLING_TOP;
            s += 1;
        }
        while s < SMP_NUM {
            this.inner[NoiseType::Rect as usize][s as usize] = -SAMPLING_TOP;
            s += 1;
        }

        let mut rng = Rng::default();
        this.inner[NoiseType::Random as usize]
            .iter_mut()
            .take(SMP_NUM_RAND as usize)
            .for_each(|p| *p = rng.next());

        for (s, p) in zip(0..SMP_NUM, &mut this.inner[NoiseType::Saw2 as usize]) {
            let ovt = overtone(osci, &overtones_saw2, s).clamp(-1.0, 1.0);
            *p = (ovt * f64::from(SAMPLING_TOP)) as i16;
        }

        for (s, p) in zip(0..SMP_NUM, &mut this.inner[NoiseType::Rect2 as usize]) {
            let ovt = overtone(osci, &overtones_rect2, s).clamp(-1.0, 1.0);
            *p = (ovt * f64::from(SAMPLING_TOP)) as i16;
        }

        for (s, p) in zip(0..SMP_NUM, &mut this.inner[NoiseType::Tri as usize]) {
            let ovt = coord(osci, &coord_tri, s, SMP_NUM).clamp(-1.0, 1.0);
            *p = (ovt * f64::from(SAMPLING_TOP)) as i16;
        }
        fill_rect3_onward(&mut this);
        this
    }
    /// (testing-only) Get the inner wave table
    #[cfg(feature = "testing")]
    #[must_use]
    pub const fn inner(&self) -> &Tables {
        &self.inner
    }
}

/// Build PCM data out of [`NoiseData`].
pub fn noise_to_pcm(noise: &mut NoiseData, table: &NoiseTable) -> PcmData {
    let sps = NATIVE_SAMPLE_RATE;
    let bps = Bps::B16;
    noise.fix();

    let unit_num = noise.get_unit_num();

    let mut nb_units = vec![NoiseBuilderUnit::default(); unit_num];
    for (nb_u, u) in zip(&mut nb_units, &noise.units) {
        build_unit(nb_u, u, &table.inner, sps);
    }
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let smp_num =
        (f64::from(noise.smp_num_44k) / (f64::from(NATIVE_SAMPLE_RATE) / f64::from(sps))) as u32;

    let mut pcm = PcmData::new();
    pcm.create(ChNum::Stereo, sps.into(), bps, smp_num);
    let mut pcm_samp = pcm.sample_mut();

    for _ in 0..smp_num {
        for c in 0..2 {
            pcm_samp = build_pcm_samp(pcm_samp, &nb_units, c, bps);
        }

        for unit in &mut nb_units {
            build_unit_noise(unit, &table.inner[NoiseType::Random as usize]);
        }
    }

    pcm
}

#[must_use]
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn build_pcm_samp<'a>(
    buf: &'a mut [u8],
    units: &[NoiseBuilderUnit<'_>],
    channel: usize,
    bps: Bps,
) -> &'a mut [u8] {
    let mut offset: i32;
    let mut work: f64;
    let mut vol: f64;
    let mut store: f64;
    let mut byte4: i32;
    store = 0.;
    for unit in units {
        let mut po;

        po = &unit.main;
        match po.ran_type {
            RandomType::None => {
                offset = po.offset as i32;
                if offset >= 0 {
                    work = f64::from(po.samp[offset as usize]);
                } else {
                    work = 0.;
                }
            }
            RandomType::Saw => {
                if po.offset >= 0. {
                    work = f64::from(
                        po.rdm_start + po.rdm_margin * po.offset as i32 / i32::from(SMP_NUM),
                    );
                } else {
                    work = 0.;
                }
            }
            RandomType::Rect => {
                if po.offset >= 0. {
                    work = f64::from(po.rdm_start);
                } else {
                    work = 0.;
                }
            }
        }
        if po.reverse {
            work *= -1.0;
        }
        work *= po.volume;

        po = &unit.volu;
        match po.ran_type {
            RandomType::None => {
                offset = po.offset as i32;
                vol = f64::from(po.samp[offset as usize]);
            }
            RandomType::Saw => {
                vol =
                    f64::from(po.rdm_start + po.rdm_margin * po.offset as i32 / i32::from(SMP_NUM));
            }
            RandomType::Rect => {
                vol = f64::from(po.rdm_start);
            }
        }
        if po.reverse {
            vol *= -1.0;
        }
        vol *= po.volume;

        work = work * (vol + f64::from(SAMPLING_TOP)) / (f64::from(SAMPLING_TOP) * 2.0);
        work *= unit.pan[channel];

        if unit.enve_index < unit.enve_num {
            work *= unit.enve_mag_start
                + (unit.enve_mag_margin * f64::from(unit.enve_count)
                    / f64::from((unit.enves[unit.enve_index]).smp));
        } else {
            work *= unit.enve_mag_start;
        }
        store += work;
    }

    byte4 = store as i32;
    byte4 = byte4.clamp((-SAMPLING_TOP).into(), SAMPLING_TOP.into());
    match bps {
        Bps::B8 => {
            buf[0] = ((byte4 >> 8) + 128) as u8;
            &mut buf[1..]
        }
        Bps::B16 => {
            let buf_i16: &mut [i16] = bytemuck::cast_slice_mut(buf);
            buf_i16[0] = byte4 as i16;
            &mut buf[2..]
        }
    }
}

fn build_unit<'smp>(
    unit: &mut NoiseBuilderUnit<'smp>,
    design_unit: &NoiseDesignUnit,
    tables: &'smp Tables,
    sps: SampleRate,
) {
    unit.enve_num = design_unit.enves.len();
    unit.pan = match design_unit.pan.cmp(&0) {
        Ordering::Less => [1., (100.0 + f64::from(design_unit.pan)) / 100.],
        Ordering::Equal => [1., 1.],
        Ordering::Greater => [(100.0 - f64::from(design_unit.pan)) / 100., 1.],
    };

    unit.enves = vec![Pt::ZERO; unit.enve_num];
    for e in 0..design_unit.enves.len() {
        (unit.enves[e]).smp = i32::from(sps) * i32::from(design_unit.enves[e].x) / 1000;
        (unit.enves[e]).mag = f64::from((design_unit.enves[e]).y) / 100.;
    }
    unit.enve_index = 0;
    unit.enve_mag_start = 0.;
    unit.enve_mag_margin = 0.;
    unit.enve_count = 0;
    while unit.enve_index < unit.enve_num {
        unit.enve_mag_margin = (unit.enves[unit.enve_index]).mag - unit.enve_mag_start;
        if (unit.enves[unit.enve_index]).smp != 0 {
            break;
        }
        unit.enve_mag_start = (unit.enves[unit.enve_index]).mag;
        unit.enve_index += 1;
    }
    let tbl = &tables[design_unit.main.type_ as usize];
    set_ocsillator(
        &mut unit.main,
        &design_unit.main,
        sps,
        tbl,
        &tables[NoiseType::Random as usize],
    );
    let tbl = &tables[design_unit.freq.type_ as usize];
    set_ocsillator(
        &mut unit.freq,
        &design_unit.freq,
        sps,
        tbl,
        &tables[NoiseType::Random as usize],
    );
    let tbl = &tables[design_unit.volu.type_ as usize];
    set_ocsillator(
        &mut unit.volu,
        &design_unit.volu,
        sps,
        tbl,
        &tables[NoiseType::Random as usize],
    );
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn build_unit_noise(unit: &mut NoiseBuilderUnit<'_>, rand_tbl: &[i16]) {
    let mut fre = match unit.freq.ran_type {
        RandomType::None => {
            let offset = unit.freq.offset as usize;
            f64::from(KEY_TOP * i32::from(unit.freq.samp[offset]) / i32::from(SAMPLING_TOP))
        }
        RandomType::Saw => f64::from(
            unit.freq.rdm_start
                + unit.freq.rdm_margin * unit.freq.offset as i32 / i32::from(SMP_NUM),
        ),
        RandomType::Rect => f64::from(unit.freq.rdm_start),
    };

    if unit.freq.reverse {
        fre *= -1.0;
    }
    fre *= unit.freq.volume;
    unit.main.increment(
        unit.main.incriment * f64::from(PULSE_FREQ.get(fre as i32)),
        rand_tbl,
    );
    unit.freq.increment(unit.freq.incriment, rand_tbl);
    unit.volu.increment(unit.volu.incriment, rand_tbl);

    if unit.enve_index < unit.enve_num {
        unit.enve_count += 1;
        if unit.enve_count >= (unit.enves[unit.enve_index]).smp {
            unit.enve_count = 0;
            unit.enve_mag_start = (unit.enves[unit.enve_index]).mag;
            unit.enve_mag_margin = 0.;
            unit.enve_index += 1;
            while unit.enve_index < unit.enve_num {
                unit.enve_mag_margin = (unit.enves[unit.enve_index]).mag - unit.enve_mag_start;
                if (unit.enves[unit.enve_index]).smp != 0 {
                    break;
                }
                unit.enve_mag_start = (unit.enves[unit.enve_index]).mag;
                unit.enve_index += 1;
            }
        }
    }
}

fn fill_rect3_onward(bld: &mut NoiseTable) {
    let (fst, snd) = bld.inner[NoiseType::Rect3 as usize]
        .split_first_chunk_mut::<{ SMP_NUM_U / 3 }>()
        .unwrap();
    let rem = SMP_NUM - SMP_NUM / 3;
    fst.fill(SAMPLING_TOP);
    snd[..rem as usize].fill(-SAMPLING_TOP);
    let (fst, snd) = bld.inner[NoiseType::Rect4 as usize]
        .split_first_chunk_mut::<{ SMP_NUM_U / 4 }>()
        .unwrap();
    fst.fill(SAMPLING_TOP);
    let rem = SMP_NUM - SMP_NUM / 4;
    snd[..rem as usize].fill(-SAMPLING_TOP);
    let (fst, snd) = bld.inner[NoiseType::Rect8 as usize]
        .split_first_chunk_mut::<{ SMP_NUM_U / 8 }>()
        .unwrap();
    fst.fill(SAMPLING_TOP);
    let rem = SMP_NUM - SMP_NUM / 8;
    snd[..rem as usize].fill(-SAMPLING_TOP);
    let (fst, snd) = bld.inner[NoiseType::Rect16 as usize]
        .split_first_chunk_mut::<{ SMP_NUM_U / 16 }>()
        .unwrap();
    fst.fill(SAMPLING_TOP);
    let rem = SMP_NUM - SMP_NUM / 16;
    snd[..rem as usize].fill(-SAMPLING_TOP);
    fill_saw3(bld);
    fill_saw4(bld);
    fill_saw6(bld);
    fill_saw8(bld);
}

fn fill_saw3(bld: &mut NoiseTable) {
    let mut chunker = Chunker::new(&mut bld.inner[NoiseType::Saw3 as usize]);
    chunker.next_until::<{ SMP_NUM_U / 3 }>().fill(SAMPLING_TOP);
    chunker.next_until::<{ SMP_NUM_U * 2 / 3 }>().fill(0);
    let rem = SMP_NUM_U - chunker.head;
    chunker.rest()[..rem].fill(-SAMPLING_TOP);
}

fn fill_saw4(bld: &mut NoiseTable) {
    let [chk1, chk2, chk3, chk4, chk5, ..] =
        bld.inner[NoiseType::Saw4 as usize].as_chunks_mut::<{ SMP_NUM_U / 4 }>().0
    else {
        return;
    };
    chk1.fill(SAMPLING_TOP);
    chk2.fill(SAMPLING_TOP / 3);
    chk3.fill(-SAMPLING_TOP / 3);
    chk4.fill(-SAMPLING_TOP);
    // Yes, the final chunk has an extra value in PxTone (apparently?)
    chk5[0] = -SAMPLING_TOP;
}

fn fill_saw6(bld: &mut NoiseTable) {
    /*let mut chunker = Chunker::new(&mut bld.inner[NoiseType::Saw6 as usize]);
    chunker.next_until::<{ SMP_NUM_U / 6 }>().fill(SAMPLING_TOP);
    chunker
        .next_until::<{ SMP_NUM_U * 2 / 6 }>()
        .fill(SAMPLING_TOP - SAMPLING_TOP.wrapping_mul(2) / 5);
    chunker.next_until::<{ SMP_NUM_U * 3 / 6 }>().fill(SAMPLING_TOP / 5);
    chunker.next_until::<{ SMP_NUM_U * 4 / 6 }>().fill(-SAMPLING_TOP / 5);
    chunker
        .next_until::<{ SMP_NUM_U * 5 / 6 }>()
        .fill(-SAMPLING_TOP + SAMPLING_TOP.wrapping_mul(2) / 5);
    chunker.rest().fill(-SAMPLING_TOP);*/
    let mut slice = &mut *bld.inner[NoiseType::Saw6 as usize];
    let mut s = 0;
    while s < SMP_NUM_U / 6 {
        slice[0] = SAMPLING_TOP;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 2 / 6 {
        slice[0] = SAMPLING_TOP - SAMPLING_TOP.wrapping_mul(2) / 5;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 3 / 6 {
        slice[0] = SAMPLING_TOP / 5;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 4 / 6 {
        slice[0] = -SAMPLING_TOP / 5;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 5 / 6 {
        slice[0] = -SAMPLING_TOP + SAMPLING_TOP.wrapping_mul(2) / 5;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U {
        slice[0] = -SAMPLING_TOP;
        slice = &mut slice[1..];
        s += 1;
    }
}

fn fill_saw8(bld: &mut NoiseTable) {
    /*let mut chunker = Chunker::new(&mut bld.inner[NoiseType::Saw8 as usize]);
    chunker.next_until::<{ SMP_NUM_U / 8 }>().fill(SAMPLING_TOP);
    chunker
        .next_until::<{ SMP_NUM_U * 2 / 8 }>()
        .fill(SAMPLING_TOP - SAMPLING_TOP.wrapping_mul(2) / 7);
    chunker
        .next_until::<{ SMP_NUM_U * 3 / 8 }>()
        .fill(SAMPLING_TOP - SAMPLING_TOP.wrapping_mul(4) / 7);
    chunker.next_until::<{ SMP_NUM_U * 4 / 8 }>().fill(SAMPLING_TOP / 7);
    chunker.next_until::<{ SMP_NUM_U * 5 / 8 }>().fill(-SAMPLING_TOP / 7);
    chunker
        .next_until::<{ SMP_NUM_U * 6 / 8 }>()
        .fill(-SAMPLING_TOP + SAMPLING_TOP.wrapping_mul(4) / 7);
    chunker
        .next_until::<{ SMP_NUM_U * 7 / 8 }>()
        .fill(-SAMPLING_TOP + SAMPLING_TOP.wrapping_mul(2) / 7);
    chunker.rest().fill(-SAMPLING_TOP);*/
    let mut slice = &mut *bld.inner[NoiseType::Saw8 as usize];
    let mut s = 0;
    while s < SMP_NUM_U / 8 {
        slice[0] = SAMPLING_TOP;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 2 / 8 {
        slice[0] = SAMPLING_TOP - SAMPLING_TOP.wrapping_mul(2) / 7;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 3 / 8 {
        slice[0] = SAMPLING_TOP - SAMPLING_TOP.wrapping_mul(4) / 7;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 4 / 8 {
        slice[0] = SAMPLING_TOP / 7;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 5 / 8 {
        slice[0] = -SAMPLING_TOP / 7;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 6 / 8 {
        slice[0] = -SAMPLING_TOP + SAMPLING_TOP.wrapping_mul(4) / 7;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U * 7 / 8 {
        slice[0] = -SAMPLING_TOP + SAMPLING_TOP.wrapping_mul(2) / 7;
        slice = &mut slice[1..];
        s += 1;
    }
    while s < SMP_NUM_U {
        slice[0] = -SAMPLING_TOP;
        slice = &mut slice[1..];
        s += 1;
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Default)]
enum RandomType {
    #[default]
    None = 0,
    Saw,
    Rect,
}

#[derive(Default, Clone)]
struct Oscillator<'s> {
    incriment: f64,
    offset: f64,
    volume: f64,
    samp: &'s [i16],
    reverse: bool,
    ran_type: RandomType,
    rdm_start: i32,
    rdm_margin: i32,
    rdm_index: usize,
}

impl Oscillator<'_> {
    fn increment(&mut self, by: f64, rand_tbl: &[i16]) {
        self.offset += by;
        if self.offset > f64::from(SMP_NUM) {
            self.offset -= f64::from(SMP_NUM);
            if self.offset >= f64::from(SMP_NUM) {
                self.offset = 0.;
            }

            if self.ran_type != RandomType::None {
                let p = rand_tbl;
                self.rdm_start = i32::from(p[self.rdm_index]);
                self.rdm_index += 1;
                if self.rdm_index >= usize::from(SMP_NUM_RAND) {
                    self.rdm_index = 0;
                }
                self.rdm_margin = i32::from(p[self.rdm_index]) - self.rdm_start;
            }
        }
    }
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn set_ocsillator<'smp>(
    to: &mut Oscillator<'smp>,
    from: &NoiseDesignOscillator,
    sps: SampleRate,
    tbl: &'smp [i16],
    rand_tbl: &[i16],
) {
    match from.type_ {
        NoiseType::Random => {
            to.ran_type = RandomType::Saw;
        }

        NoiseType::Random2 => {
            to.ran_type = RandomType::Rect;
        }
        _ => {
            to.ran_type = RandomType::None;
        }
    }

    to.incriment = (f64::from(NATIVE_SAMPLE_RATE) / f64::from(sps))
        * (f64::from(from.freq) / f64::from(BASIC_FREQUENCY));

    if to.ran_type == RandomType::None {
        to.offset = f64::from(SMP_NUM) * f64::from(from.offset / 100.);
    } else {
        to.offset = 0.;
    }

    to.volume = f64::from(from.volume / 100.);
    to.samp = tbl;
    to.reverse = from.invert;

    to.rdm_start = 0;
    to.rdm_index = (f64::from(SMP_NUM_RAND) * f64::from(from.offset / 100.)) as usize;
    let p = rand_tbl;
    to.rdm_margin = i32::from(p[to.rdm_index]);
}

const BASIC_FREQUENCY: u8 = 100;
const SAMPLING_TOP: i16 = 32767;
const KEY_TOP: i32 = 0x3200;
const SMP_NUM_RAND: SampleRate = NATIVE_SAMPLE_RATE;
const SMP_NUM: u16 = NATIVE_SAMPLE_RATE / BASIC_FREQUENCY as u16;
const SMP_NUM_U: usize = SMP_NUM as usize;

#[derive(Default, Clone)]
struct NoiseBuilderUnit<'smp> {
    pan: [f64; 2],
    enve_index: usize,
    enve_mag_start: f64,
    enve_mag_margin: f64,
    enve_count: i32,
    enve_num: usize,
    enves: Vec<Pt>,
    main: Oscillator<'smp>,
    freq: Oscillator<'smp>,
    volu: Oscillator<'smp>,
}

#[derive(Clone)]
struct Pt {
    smp: i32,
    mag: f64,
}

impl Pt {
    const ZERO: Self = Self { smp: 0, mag: 0.0 };
}

/// Types of waves for noise generation
#[expect(missing_docs)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum NoiseType {
    #[default]
    Sine,
    Saw,
    Rect,
    Random,
    Saw2,
    Rect2,
    Tri,
    Random2,
    Rect3,
    Rect4,
    Rect8,
    Rect16,
    Saw3,
    Saw4,
    Saw6,
    Saw8,
}

/// An oscillator for generating different kinds of noise waveforms.
#[derive(Copy, Clone, Default)]
pub struct NoiseDesignOscillator {
    /// The type of wave to use
    pub type_: NoiseType,
    /// Frequency at which we oscillate
    pub freq: f32,
    /// Volume
    pub volume: f32,
    /// Offset
    pub offset: f32,
    /// Invert the waveform
    pub invert: bool,
}
