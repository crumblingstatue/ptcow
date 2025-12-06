const OCTAVE_NUM: u8 = 16;
const KEY_PER_OCTAVE: u8 = 12;
const FREQUENCY_PER_KEY: u8 = 0x10;
const TABLE_SIZE: usize =
    OCTAVE_NUM as usize * KEY_PER_OCTAVE as usize * FREQUENCY_PER_KEY as usize;

pub struct PulseFrequency {
    table: [f32; TABLE_SIZE],
}

pub static PULSE_FREQ: PulseFrequency = PulseFrequency::generate();

impl PulseFrequency {
    const fn generate() -> Self {
        #[expect(clippy::unreadable_literal)]
        let oct_table: [f64; OCTAVE_NUM as usize] = [
            0.00390625, 0.0078125, 0.015625, 0.03125, 0.0625, 0.125, 0.25, 0.5, 1., 2., 4., 8.,
            16., 32., 64., 128.,
        ];

        let mut table = [0.0; _];

        let oct_x24: f64 = divide_octave_rate(KEY_PER_OCTAVE * FREQUENCY_PER_KEY);

        let mut i = 0;
        while i < table.len() {
            let freq = &mut table[i];
            let mut oct = oct_table[i / (KEY_PER_OCTAVE as usize * FREQUENCY_PER_KEY as usize)];
            let mut j = 0;
            while j < i % (KEY_PER_OCTAVE as usize * FREQUENCY_PER_KEY as usize) {
                oct *= oct_x24;
                j += 1;
            }
            #[expect(clippy::cast_possible_truncation)]
            (*freq = oct as f32);
            i += 1;
        }
        Self { table }
    }
    pub const fn get(&self, key: usize) -> f32 {
        let mut i = (key.wrapping_add(0x6000)).wrapping_mul(FREQUENCY_PER_KEY as usize) / 0x100;
        if i >= TABLE_SIZE {
            i = TABLE_SIZE - 1;
        }
        self.table[i]
    }
    pub const fn get2(&self, key: usize) -> f32 {
        let mut i = key >> 4;
        if i >= TABLE_SIZE {
            i = TABLE_SIZE - 1;
        }
        self.table[i]
    }
}

const fn divide_octave_rate(divi: u8) -> f64 {
    let mut parameter: f64 = 1.0;

    let mut i = 0;
    while i < 17 {
        let mut add = 1.0;
        let mut j = 0;
        while j < i {
            add *= 0.1;
            j += 1;
        }

        j = 0;

        while j < 10 {
            let work = (add * j as f64) + parameter;

            let mut result = 1.0;
            let mut k = 0;
            while k < divi {
                result *= work;
                if result >= 2.0 {
                    break;
                }
                k += 1;
            }

            if k != divi {
                break;
            }
            j += 1;
        }
        parameter += add * (j as f64 - 1.0);
        i += 1;
    }

    parameter
}
