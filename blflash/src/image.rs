use std::io::Read;

pub struct BinReader<R> {
    reader: R,
}

impl<R> BinReader<R>
where
    R: Read,
{
    pub fn new(reader: R) -> BinReader<R> {
        BinReader {
            reader,
        }
    }
}
