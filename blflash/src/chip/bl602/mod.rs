use super::Chip;

pub const EFLASH_LOADER: &'static [u8] = include_bytes!("eflash_loader_40m.bin");

pub struct Bl602;

impl Chip for Bl602 {
    fn get_eflash_loader(&self) -> &[u8] {
        EFLASH_LOADER
    }
}
