use crate::Error;
use byteorder::{NativeEndian, ReadBytesExt};
use deku::prelude::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct BootHeaderCfgFile {
    #[serde(rename = "BOOTHEADER_CFG")]
    pub boot_header_cfg: BootHeaderCfg,
}

#[derive(Debug, Deserialize, DekuWrite, Default, Clone)]
pub struct FlashCfg {
    flashcfg_magic_code: u32,
    // 12
    io_mode: u8,
    cont_read_support: u8,
    sfctrl_clk_delay: u8,
    sfctrl_clk_invert: u8,
    // 16
    reset_en_cmd: u8,
    reset_cmd: u8,
    exit_contread_cmd: u8,
    exit_contread_cmd_size: u8,
    // 20
    jedecid_cmd: u8,
    jedecid_cmd_dmy_clk: u8,
    qpi_jedecid_cmd: u8,
    qpi_jedecid_dmy_clk: u8,
    // 24
    sector_size: u8,
    mfg_id: u8,
    page_size: u16,
    // 28
    chip_erase_cmd: u8,
    sector_erase_cmd: u8,
    blk32k_erase_cmd: u8,
    blk64k_erase_cmd: u8,
    // 32
    write_enable_cmd: u8,
    page_prog_cmd: u8,
    qpage_prog_cmd: u8,
    qual_page_prog_addr_mode: u8,
    // 36
    fast_read_cmd: u8,
    fast_read_dmy_clk: u8,
    qpi_fast_read_cmd: u8,
    qpi_fast_read_dmy_clk: u8,
    // 40
    fast_read_do_cmd: u8,
    fast_read_do_dmy_clk: u8,
    fast_read_dio_cmd: u8,
    fast_read_dio_dmy_clk: u8,
    // 44
    fast_read_qo_cmd: u8,
    fast_read_qo_dmy_clk: u8,
    fast_read_qio_cmd: u8,
    fast_read_qio_dmy_clk: u8,
    // 48
    qpi_fast_read_qio_cmd: u8,
    qpi_fast_read_qio_dmy_clk: u8,
    qpi_page_prog_cmd: u8,
    write_vreg_enable_cmd: u8,
    // 52
    wel_reg_index: u8,
    qe_reg_index: u8,
    busy_reg_index: u8,
    wel_bit_pos: u8,
    // 56
    qe_bit_pos: u8,
    busy_bit_pos: u8,
    wel_reg_write_len: u8,
    wel_reg_read_len: u8,
    // 60
    qe_reg_write_len: u8,
    qe_reg_read_len: u8,
    release_power_down: u8,
    busy_reg_read_len: u8,
    // 64
    reg_read_cmd0: u8,
    reg_read_cmd1: u8,
    #[serde(skip)]
    _unused1: u16,
    // 68
    reg_write_cmd0: u8,
    reg_write_cmd1: u8,
    #[serde(skip)]
    _unused2: u16,
    // 72
    enter_qpi_cmd: u8,
    exit_qpi_cmd: u8,
    cont_read_code: u8,
    cont_read_exit_code: u8,
    // 76
    burst_wrap_cmd: u8,
    burst_wrap_dmy_clk: u8,
    burst_wrap_data_mode: u8,
    burst_wrap_code: u8,
    // 80
    de_burst_wrap_cmd: u8,
    de_burst_wrap_cmd_dmy_clk: u8,
    de_burst_wrap_code_mode: u8,
    de_burst_wrap_code: u8,
    // 84
    sector_erase_time: u16,
    blk32k_erase_time: u16,
    // 88
    blk64k_erase_time: u16,
    page_prog_time: u16,
    // 92
    chip_erase_time: u16,
    power_down_delay: u8,
    qe_data: u8,
    // 96
    #[deku(update = "self.checksum()")]
    flashcfg_crc32: u32,
}

#[derive(Debug, Deserialize, DekuWrite, Default, Clone)]
pub struct ClkCfg {
    // 100
    clkcfg_magic_code: u32,
    // 104
    xtal_type: u8,
    pll_clk: u8,
    hclk_div: u8,
    bclk_div: u8,
    // 108
    flash_clk_type: u8,
    flash_clk_div: u8,
    #[serde(skip)]
    _unused1: u16,
    // 112
    #[deku(update = "self.checksum()")]
    clkcfg_crc32: u32,
}

// NOTE: the order is reversed here
// see: https://github.com/sharksforarms/deku/issues/134
#[derive(Debug, Deserialize, DekuWrite, Default, Clone)]
pub struct BootCfg {
    // 116
    #[deku(bits = 2)]
    #[serde(skip)]
    _unused1: u8,
    #[deku(bits = 2)]
    key_sel: u8,
    #[deku(bits = 2)]
    encrypt_type: u8,
    #[deku(bits = 2)]
    sign: u8,
    // 117
    #[deku(bits = 4)]
    cache_way_disable: u8,
    #[deku(bits = 1)]
    aes_region_lock: u8,
    #[deku(bits = 1)]
    notload_in_bootrom: u8,
    #[deku(bits = 1)]
    cache_enable: u8,
    #[deku(bits = 1)]
    no_segment: u8,
    // 118
    #[deku(bits = 14)]
    #[serde(skip)]
    _unused2: u32,
    #[deku(bits = 1)]
    hash_ignore: u8,
    #[deku(bits = 1)]
    crc_ignore: u8,

    // 120
    pub img_len: u32,
    // 124
    bootentry: u32,
    // 128
    img_start: u32,
    // 132
    hash_0: u32,
    hash_1: u32,
    hash_2: u32,
    hash_3: u32,
    hash_4: u32,
    hash_5: u32,
    hash_6: u32,
    hash_7: u32,

    #[serde(skip)]
    _unused3: [u8; 8],
}

#[derive(Debug, Deserialize, DekuWrite, Default, Clone)]
pub struct BootHeaderCfg {
    magic_code: u32,
    revision: u32,

    #[serde(flatten)]
    pub flash_cfg: FlashCfg,

    #[serde(flatten)]
    pub clk_cfg: ClkCfg,

    #[serde(flatten)]
    pub boot_cfg: BootCfg,

    // 172
    #[deku(update = "self.checksum()")]
    crc32: u32,
}

impl FlashCfg {
    fn checksum(&self) -> u32 {
        let data = self.to_bytes().unwrap();
        crc::crc32::checksum_ieee(&data[4..data.len() - 4])
    }
}

impl ClkCfg {
    fn checksum(&self) -> u32 {
        let data = self.to_bytes().unwrap();
        crc::crc32::checksum_ieee(&data[4..data.len() - 4])
    }
}

impl BootHeaderCfg {
    fn checksum(&self) -> u32 {
        let data = self.to_bytes().unwrap();
        crc::crc32::checksum_ieee(&data[0..data.len() - 4])
    }
    fn update_sha256(&mut self, hash: &[u8]) -> Result<(), Error> {
        let mut reader = Cursor::new(hash);
        self.boot_cfg.hash_0 = reader.read_u32::<NativeEndian>()?;
        self.boot_cfg.hash_1 = reader.read_u32::<NativeEndian>()?;
        self.boot_cfg.hash_2 = reader.read_u32::<NativeEndian>()?;
        self.boot_cfg.hash_3 = reader.read_u32::<NativeEndian>()?;
        self.boot_cfg.hash_4 = reader.read_u32::<NativeEndian>()?;
        self.boot_cfg.hash_5 = reader.read_u32::<NativeEndian>()?;
        self.boot_cfg.hash_6 = reader.read_u32::<NativeEndian>()?;
        self.boot_cfg.hash_7 = reader.read_u32::<NativeEndian>()?;
        Ok(())
    }
    pub fn make_image(&mut self, offset: usize, mut image: Vec<u8>) -> Result<Vec<u8>, Error> {
        let binlen = ((image.len() + 15) / 16) * 16;
        image.resize(binlen, 0xFF);
        let hash = Sha256::digest(&image);
        self.update_sha256(&hash[..])?;
        self.boot_cfg.img_len = image.len() as u32;
        self.flash_cfg.update()?;
        self.clk_cfg.update()?;
        self.update()?;

        let mut header = self.to_bytes()?;

        header.resize(offset, 0xff);
        header.append(&mut image);

        Ok(header)
    }
}
