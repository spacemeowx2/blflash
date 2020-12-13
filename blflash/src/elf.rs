use std::borrow::Cow;
use std::cmp::Ordering;

use xmas_elf::program::{SegmentData, Type};
use xmas_elf::ElfFile;

use crate::chip::Chip;

pub struct FirmwareImage<'a> {
    pub entry: u32,
    pub elf: ElfFile<'a>,
}

impl<'a> FirmwareImage<'a> {
    pub fn from_data(data: &'a [u8]) -> Result<Self, &'static str> {
        Ok(Self::from_elf(ElfFile::new(data)?))
    }

    pub fn from_elf(elf: ElfFile<'a>) -> Self {
        FirmwareImage {
            entry: elf.header.pt2.entry_point() as u32,
            elf,
        }
    }

    pub fn entry(&self) -> u32 {
        self.elf.header.pt2.entry_point() as u32
    }

    pub fn segments(&'a self) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
        self.elf
            .program_iter()
            .filter(|header| {
                header.file_size() > 0 && header.get_type() == Ok(Type::Load) && header.offset() > 0
            })
            .flat_map(move |header| {
                let addr = header.physical_addr() as u32;
                let size = header.file_size() as u32;
                let data = match header.get_data(&self.elf) {
                    Ok(SegmentData::Undefined(data)) => data,
                    _ => return None,
                };
                Some(CodeSegment { addr, data, size })
            })
    }
    pub fn to_flash_bin(&self, chip: &dyn Chip) -> Vec<u8> {
        let segs = self
            .segments()
            .filter_map(|segment| chip.get_flash_segment(segment))
            .collect::<Vec<_>>();
        let size = segs
            .iter()
            .fold(0, |len, i| len.max(i.addr + i.data.len() as u32));

        let mut bin = Vec::new();
        bin.resize(size as usize, 0xFF);
        for s in segs {
            bin[s.addr as usize..s.addr as usize + s.data.len()].copy_from_slice(&s.data);
        }
        bin
    }
}

#[derive(Debug, Ord, Eq)]
/// A segment of code from the source elf
pub struct CodeSegment<'a> {
    pub addr: u32,
    pub size: u32,
    pub data: &'a [u8],
}

impl<'a> CodeSegment<'a> {
    pub fn from_slice<D: AsRef<[u8]>>(addr: u32, data: &'a D) -> Self {
        let data = data.as_ref();
        CodeSegment {
            addr,
            data: &data,
            size: data.len() as u32,
        }
    }
}

impl<'a> AsRef<[u8]> for CodeSegment<'a> {
    fn as_ref(&self) -> &[u8] {
        self.data
    }
}

impl PartialEq for CodeSegment<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.addr.eq(&other.addr)
    }
}

impl PartialOrd for CodeSegment<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.addr.partial_cmp(&other.addr)
    }
}

/// A segment of data to write to the flash
pub struct RomSegment<'a> {
    pub addr: u32,
    pub data: Cow<'a, [u8]>,
}

impl<'a> RomSegment<'a> {
    pub fn size(&self) -> u32 {
        self.data.len() as u32
    }
    pub fn from_vec(addr: u32, data: Vec<u8>) -> Self {
        RomSegment {
            addr,
            data: Cow::Owned(data),
        }
    }
    pub fn from_slice(addr: u32, data: &'a [u8]) -> RomSegment<'a> {
        RomSegment {
            addr,
            data: Cow::Borrowed(data),
        }
    }
    pub fn from_code_segment(addr: u32, code_segment: CodeSegment<'a>) -> RomSegment<'a> {
        Self {
            addr,
            data: Cow::Borrowed(code_segment.data),
        }
    }
}
