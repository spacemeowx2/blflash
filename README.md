# blflash

BL602 serial flasher

The FTDI ft232rl can be used to auto-triggering the bootloader before flashing. If using this module make these additional connections:

| ft232rl | bl602 |
| --- | ---|
| DTR | EN |
| RTS | D8 |

Inspired by https://github.com/esp-rs/espflash, https://github.com/bouffalolab/BLOpenFlasher

ISP documentation: https://github.com/bouffalolab/bl_docs/tree/main/BL602_ISP

## TODO

- [x] Flash protocol
- [x] Generate Partition bin
- [x] Generate boot info with compiled bin
- [ ] Generate dtb bin
