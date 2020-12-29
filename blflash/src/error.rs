use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("IO error while using serial port: {0}")]
    Serial(#[from] serial::core::Error),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Failed to connect to the device")]
    ConnectionFailed,
    #[error("Timeout while running command")]
    Timeout,
    #[error("Invalid response header")]
    RespError,
    #[error("Packet to large for buffer")]
    OverSizedPacket,
    #[error("elf image is not valid")]
    InvalidElf,
    #[error("elf image can not be ran from ram")]
    ElfNotRamLoadable,
    #[error("chip not recognized")]
    UnrecognizedChip,
    #[error("flash chip not supported, flash id: {0:#x}")]
    UnsupportedFlash(u8),
    #[error("ROM error {0:?}")]
    RomError(RomError),
    #[error("Parse error")]
    ParseError(#[from] deku::error::DekuError),
    #[error("Parse toml error")]
    TomlError(#[from] toml::de::Error),
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum RomError {
    Success,
    Other(u16),
}

impl From<u16> for RomError {
    fn from(raw: u16) -> Self {
        match raw {
            0x00 => RomError::Success,
            _ => RomError::Other(raw),
        }
    }
}
