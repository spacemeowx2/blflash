use super::{Chip, CodeSegment, RomSegment};
use crate::{
    image::{BootHeaderCfg, PartitionCfg},
    Error,
};
use deku::prelude::*;

pub const DEFAULT_PARTITION_CFG: &'static [u8] = include_bytes!("cfg/partition_cfg_2M.toml");
pub const DEFAULT_BOOTHEADER_CFG: &'static [u8] = include_bytes!("cfg/efuse_bootheader_cfg.conf");
pub const RO_PARAMS: &'static [u8] = include_bytes!("cfg/ro_params.dtb");
pub const BLSP_BOOT2: &'static [u8] = include_bytes!("image/blsp_boot2.bin");
pub const EFLASH_LOADER: &'static [u8] = include_bytes!("image/eflash_loader_40m.bin");
const ROM_START: u32 = 0x23000000;
// 16MB
const ROM_END: u32 = 0x23000000 + 0x1000000;

#[derive(Copy, Clone)]
pub struct Bl602;

impl Bl602 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        addr >= ROM_START && addr < ROM_END
    }
}

impl Chip for Bl602 {
    fn target(&self) -> &'static str {
        "riscv32imac-unknown-none-elf"
    }

    fn get_eflash_loader(&self) -> &[u8] {
        EFLASH_LOADER
    }

    fn get_flash_segment<'a>(&self, code_segment: CodeSegment<'a>) -> Option<RomSegment<'a>> {
        if self.addr_is_flash(code_segment.addr) {
            Some(RomSegment::from_code_segment(
                code_segment.addr - ROM_START,
                code_segment,
            ))
        } else {
            None
        }
    }

    fn with_boot2(
        &self,
        mut partition_cfg: PartitionCfg,
        mut bootheader_cfg: BootHeaderCfg,
        ro_params: Vec<u8>,
        bin: &[u8],
    ) -> Result<Vec<RomSegment>, Error> {
        partition_cfg.update()?;
        let partition_cfg = partition_cfg.to_bytes()?;

        let boot2image = bootheader_cfg.make_image(0x2000, Vec::from(BLSP_BOOT2))?;
        let fw_image = bootheader_cfg.make_image(0x1000, Vec::from(bin))?;

        let segments = vec![
            RomSegment::from_vec(0x0, boot2image),
            RomSegment::from_vec(0xe000, partition_cfg.clone()),
            RomSegment::from_vec(0xf000, partition_cfg),
            RomSegment::from_vec(0x10000, fw_image),
            // TODO: generate from dts
            RomSegment::from_vec(0x1f8000, ro_params),
        ];

        Ok(segments)
    }
}
