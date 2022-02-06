# blflash

BL602 serial flasher

blflash is written in rust. It requires the latest environment of rust, which
you can get via running 'rustup update'. You can build blflash by running
'cargo build'.

Inspired by https://github.com/esp-rs/espflash, https://github.com/bouffalolab/BLOpenFlasher

ISP documentation: https://github.com/bouffalolab/bl_docs/tree/main/BL602_ISP

## TODO

- [x] Flash protocol
- [x] Generate Partition bin
- [x] Generate boot info with compiled bin
- [ ] Generate dtb bin
