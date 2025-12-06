use {crate::result::ProjectReadError, arrayvec::ArrayVec};

pub struct Reader<'a> {
    pub data: &'a [u8],
    pub cur: usize,
}

#[derive(Debug)]
pub struct ReadError;

impl From<ReadError> for ProjectReadError {
    fn from(ReadError: ReadError) -> Self {
        Self::Data
    }
}

impl Reader<'_> {
    pub fn next<T: bytemuck::AnyBitPattern>(&mut self) -> Result<T, ReadError> {
        let amount = size_of::<T>();
        let bytes = self.data.get(self.cur..self.cur + amount).ok_or(ReadError)?;
        self.cur += amount;
        Ok(bytemuck::pod_read_unaligned(bytes))
    }
    pub fn fill_slice(&mut self, dst: &mut [u8]) -> Result<(), ReadError> {
        let amount = dst.len();
        let Some(src) = self.data.get(self.cur..self.cur + amount) else {
            return Err(ReadError);
        };
        if src.len() != dst.len() {
            return Err(ReadError);
        }
        dst.copy_from_slice(src);
        self.cur += amount;
        Ok(())
    }
    pub fn next_varint(&mut self) -> Result<u32, ReadError> {
        let mut a: VarintBuf = VarintBuf::new();
        let mut count: u8 = 0;
        while count < 5 {
            let byte = self.next()?;
            a.push(byte);

            if i32::from(a[count as usize]) & 0x80 == 0 {
                break;
            }
            count += 1;
        }
        varint_to_int(&a).ok_or(ReadError)
    }
}

type VarintBuf = ArrayVec<u8, 5>;

fn varint_to_int(buf: &VarintBuf) -> Option<u32> {
    let mut b = [0; 4];
    match buf.len() {
        1 => {
            b[0] = (buf[0]) & 0x7f;
        }
        2 => {
            b[0] = ((buf[0]) & 0x7f) | (buf[1]) << 7;
            b[1] = ((buf[1]) & 0x7f) >> 1;
        }
        3 => {
            b[0] = ((buf[0]) & 0x7f) | (buf[1]) << 7;
            b[1] = ((buf[1]) & 0x7f) >> 1 | (buf[2]) << 6;
            b[2] = ((buf[2]) & 0x7f) >> 2;
        }
        4 => {
            b[0] = ((buf[0]) & 0x7f) | (buf[1]) << 7;
            b[1] = ((buf[1]) & 0x7f) >> 1 | (buf[2]) << 6;
            b[2] = ((buf[2]) & 0x7f) >> 2 | (buf[3]) << 5;
            b[3] = ((buf[3]) & 0x7f) >> 3;
        }
        5 => {
            b[0] = ((buf[0]) & 0x7f) | (buf[1]) << 7;
            b[1] = ((buf[1]) & 0x7f) >> 1 | (buf[2]) << 6;
            b[2] = ((buf[2]) & 0x7f) >> 2 | (buf[3]) << 5;
            b[3] = ((buf[3]) & 0x7f) >> 3 | (buf[4]) << 4;
        }
        _ => return None,
    }
    Some(u32::from_le_bytes(b))
}

fn int_to_varint(num: u32) -> ArrayVec<u8, 5> {
    let mut out = ArrayVec::new();
    let a = num.to_le_bytes();
    if num < 0x80 {
        out.push(a[0]);
    } else if num < 0x4000 {
        out.push((a[0] & 0x7F) | 0x80);
        out.push(a[0] >> 7 | ((a[1] << 1) & 0x7F));
    } else if num < 0x20_0000 {
        out.push((a[0] & 0x7F) | 0x80);
        out.push((a[0] >> 7) | ((a[1] << 1) & 0x7F) | 0x80);
        out.push((a[1] >> 6) | ((a[2] << 2) & 0x7F));
    } else if num < 0x1000_0000 {
        out.push((a[0] & 0x7F) | 0x80);
        out.push((a[0] >> 7) | ((a[1] << 1) & 0x7F) | 0x80);
        out.push((a[1] >> 6) | ((a[2] << 2) & 0x7F) | 0x80);
        out.push((a[2] >> 5) | ((a[3] << 3) & 0x7F));
    } else {
        out.push((a[0] & 0x7F) | 0x80);
        out.push((a[0] >> 7) | ((a[1] << 1) & 0x7F) | 0x80);
        out.push((a[1] >> 6) | ((a[2] << 2) & 0x7F) | 0x80);
        out.push((a[2] >> 5) | ((a[3] << 3) & 0x7F) | 0x80);
        out.push(a[3] >> 4);
    }
    out
}

#[test]
fn test_varint_equiv() {
    for i in (0..u32::MAX).step_by(0x1234) {
        let v = int_to_varint(i);
        let n = varint_to_int(&v).unwrap();
        assert_eq!(i, n);
    }
    // Just to make sure it doesn't fail on odd numbers
    for i in (0..u32::MAX).step_by(0x1233) {
        let v = int_to_varint(i);
        let n = varint_to_int(&v).unwrap();
        assert_eq!(i, n);
    }
}

pub fn write_varint(num: u32, out: &mut Vec<u8>) {
    let v_int = int_to_varint(num);
    out.extend_from_slice(&v_int);
}
