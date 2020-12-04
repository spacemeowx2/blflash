use super::{Chip, CodeSegment, FlashSegment};

const EFLASH_LOADER: &'static [u8] = include_bytes!("eflash_loader_40m.bin");
const ROM_START: u32 = 0x23000000;
// 16MB
const ROM_END: u32 = 0x23000000 + 0x1000000;

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

    fn get_flash_segment<'a>(&self, code_segment: CodeSegment<'a>) -> Option<FlashSegment<'a>> {
        if self.addr_is_flash(code_segment.addr) {
            Some(FlashSegment {
                addr: code_segment.addr - ROM_START,
                code_segment,
            })
        } else {
            None
        }
    }
}
