mod bl602;
pub use bl602::Bl602;

pub trait Chip {
    fn get_eflash_loader(&self) -> &[u8];
}
