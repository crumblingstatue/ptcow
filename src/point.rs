/// An envelope point
#[derive(Copy, Clone, Default, Debug)]
pub struct EnvPt {
    /// X offset from previous point
    pub x: u16,
    /// Volume
    pub y: u8,
}

impl EnvPt {
    /// `[0, 0]` coordinate
    pub const ZERO: Self = Self { x: 0, y: 0 };
}
