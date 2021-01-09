import init, { dump, flash, FS } from './pkg/libblflash.js'

async function onDump() {
    try {
        await dump({
            port: 'port',
            baud_rate: 1000000,
            initial_baud_rate: 115200,
            output: "output.bin",
            start: 0,
            end: 0x100000,
        })
        console.log('done')
        debugger
        const result = FS.read_file('output.bin')
        console.log(result)
    } catch(e) {
        console.error('error during Dump', e)
    }
}
async function onFlash(event) {
    try {
        const file = document.getElementById('file').files[0]
        const content = await new Response(file).arrayBuffer()

        FS.write_file('input.bin', new Uint8Array(content))
        await flash({
            port: 'port',
            baud_rate: 1000000,
            initial_baud_rate: 115200,
            image: 'input.bin',
        })
        console.log('done')
    } catch(e) {
        console.error('error during Flash', e)
    }
}
async function main() {
    console.log('load wasm')
    await init()
    console.log('wasm loaded')
    document.getElementById('dump').addEventListener('click', onDump)
    document.getElementById('flash').addEventListener('click', onFlash)
}
main()
