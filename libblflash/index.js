import init, { dump, FS } from './pkg/libblflash.js'

async function onDump() {
    await dump({
        port: 'port',
        baud_rate: 115200,
        initial_baud_rate: 115200,
        output: "output.bin",
        start: 0,
        end: 0x100000,
    })
}
async function main() {
    console.log('load wasm')
    await init()
    console.log('wasm loaded')

    // const r = await dump({
    //     port: 'port',
    //     baud_rate: 115200,
    //     initial_baud_rate: 115200,
    //     output: "output.bin",
    //     start: 0,
    //     end: 0x100000,
    // })
    // console.log(r)
}
main()
document.getElementById('dump').addEventListener('click', onDump)