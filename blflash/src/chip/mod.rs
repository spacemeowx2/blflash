pub mod bl602;
pub use bl602::Bl602;
pub use crate::elf::{FirmwareImage, CodeSegment};
pub use crate::flasher::FlashSegment;

pub trait Chip {
    fn get_eflash_loader(&self) -> &[u8];
    fn get_flash_segment<'a>(&self, code_segment: CodeSegment<'a>) -> Option<FlashSegment<'a>>;
}
