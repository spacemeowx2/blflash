use super::{Chip, CodeSegment, FirmwareImage};

const EFLASH_LOADER: &'static [u8] = include_bytes!("eflash_loader_40m.bin");
const ROM_START: u32 = 0x21000000;
const ROM_END: u32 = 0x21000000 + 0x20000;

pub struct Bl602;

impl Bl602 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        addr >= ROM_START && addr < ROM_END
    }
}

impl Chip for Bl602 {
    fn get_eflash_loader(&self) -> &[u8] {
        EFLASH_LOADER
    }

    fn get_flash_segments<'a>(&self, image: &'a FirmwareImage) -> Vec<CodeSegment<'a>> {
        image.segments().filter_map(|s| {
            if self.addr_is_flash(s.addr) {
                Some(CodeSegment {
                    addr: s.addr - ROM_START,
                    ..s
                })
            } else {
                None
            }
        }).collect()
    }
}
