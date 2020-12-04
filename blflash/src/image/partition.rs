use deku::prelude::*;
use serde::Deserialize;
use std::io::Write;
use std::iter;

#[derive(Debug, Deserialize, DekuWrite, Default)]
#[deku(magic = b"\x42\x46\x50\x54\x00\x00")]
pub struct PartitionCfg {
    #[serde(skip)]
    #[deku(update = "self.pt_entry.len()")]
    pub entry_len: u32,
    #[serde(skip)]
    #[deku(update = "0")]
    _unused1: u16,
    #[serde(skip)]
    #[deku(update = "self.header_checksum()")]
    pub checksum: u32,
    #[deku(skip)]
    pub pt_table: Table,
    pub pt_entry: Vec<Entry>,
    #[serde(skip)]
    #[deku(update = "self.checksum()")]
    pub file_checksum: u32,
}

#[derive(Debug, Deserialize, DekuWrite, Default)]
pub struct Table {
    pub address0: u32,
    pub address1: u32,
}

#[derive(Debug, Deserialize, DekuWrite, Default)]
pub struct Entry {
    #[deku(bytes = "3")]
    pub r#type: u32,
    #[deku(writer = "Entry::write_name(name, output)")]
    pub name: String,
    pub address0: u32,
    pub address1: u32,
    pub size0: u32,
    pub size1: u32,
    pub len: u32,
    #[serde(skip)]
    pub _unused1: u32,
}

impl PartitionCfg {
    fn header_checksum(&self) -> u32 {
        let data = self.to_bytes().unwrap();
        crc::crc32::checksum_ieee(&data[0..12])
    }
    fn checksum(&self) -> u32 {
        let data = self.to_bytes().unwrap();
        crc::crc32::checksum_ieee(&data[16..16 + 36 * self.pt_entry.len()])
    }
}

impl Entry {
    fn write_name(name: &str, output: &mut BitVec<Msb0, u8>) -> Result<(), DekuError> {
        if name.len() > 8 {
            return Err(DekuError::Unexpected("name too long".to_string()));
        }
        let bytes = name
            .bytes()
            .chain(iter::repeat(0))
            .take(8 + 1) // last is null?
            .collect::<Vec<_>>();
        output.write_all(&bytes).unwrap();
        Ok(())
    }
}
