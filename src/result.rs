use thiserror::Error;

/// Error that can happen when reading a PxTone project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProjectReadError {
    /// If a project contains this tag, it's an error.
    #[error("anti operation")]
    AntiOpreation,
    /// Low level data read error (premature EOF, varint parsing error)
    #[error("Data read error")]
    Data,
    /// Format newer than supported
    #[error("Format newer than supported")]
    FmtNewer,
    /// Unknown format
    #[error("Unknown format")]
    FmtUnknown,
    /// Invalid/unsupported tag
    #[error("Invalid/unsupported tag")]
    InvalidTag,
    /// Invalid/unsupported tag data
    #[error("Invalid/unsupported tag data")]
    InvalidData,
    /// Error reading Ogg/vorbis data
    #[error("Ogg/vorbis read error")]
    OggvReadError,
    /// ptcow was built with Ogg/vorbis support disabled
    #[error("Ogg/vorbis support disabled")]
    OggvSupportDisabled,
    /// V4 (and earlier?) relies on the event list being a linked list, which would require
    /// a lot of figuring out how to make it work with our implementation using `Vec`.
    #[error("Unsupported old PxTone version")]
    OldUnsupported,
    /// We internally store overtone points as 16 bit integers, but they are encoded
    /// as up-to 32 bit varints. Valid songs should never have overtone points this large,
    /// but an invalid song could contain such a point.
    #[error("Overtone point out of range: {0} (should be between in i32/i16 range for x/y)")]
    OvertonePointOutOfRange(u32),
}

/// Error that can happen when saving a PxTone project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProjectWriteError {
    /// We don't support writing this format
    #[error("Unsupported format for saving.")]
    UnsupportedFmt,
    /// We internally store the points as 16 bit due to various reasons, but the PxTone
    /// format only supports 8 bit points for coord waves.
    #[error("Coord wave point out of range (needs to be between 0 and 255")]
    CoordWavePointOutOfRange,
}

/// Result of attempting to read a PxTone project
pub type ReadResult<T = ()> = Result<T, ProjectReadError>;

/// Result of attempting to write a PxTone project
pub type WriteResult<T = ()> = Result<T, ProjectWriteError>;
