mod bl602;
pub use bl602::Bl602;
pub use crate::elf::{FirmwareImage, CodeSegment};

pub trait Chip {
    fn get_eflash_loader(&self) -> &[u8];
    fn get_flash_segments<'a>(&self, image: &'a FirmwareImage) -> Vec<CodeSegment<'a>>;
}
