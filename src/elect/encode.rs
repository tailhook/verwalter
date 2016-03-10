use std::io::Cursor;
use cbor::Encoder;

use super::Id;
use super::machine::Epoch;

const PING: u32 = 1;


pub fn ping(id: &Id, epoch: Epoch) -> Vec<u8> {
    let mut buf = Encoder::new(Cursor::new(Vec::new()));
    id.encode(&mut buf).unwrap();
    buf.u64(epoch).unwrap();
    buf.u32(PING).unwrap();
    return buf.into_writer().into_inner();
}
