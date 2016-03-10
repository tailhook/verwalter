use std::io::Cursor;
use cbor::{Encoder, Decoder, Config, DecodeResult};
use cbor::{DecodeError};
use cbor::types::Type;

use super::{Id, Capsule, Message};
use super::machine::Epoch;

const PING: u8 = 1;
const VOTE: u8 = 2;


pub fn ping(id: &Id, epoch: Epoch) -> Vec<u8> {
    let mut buf = Encoder::new(Cursor::new(Vec::new()));
    id.encode(&mut buf).unwrap();
    buf.u64(epoch).unwrap();
    buf.u8(PING).unwrap();
    return buf.into_writer().into_inner();
}

pub fn vote(id: &Id, epoch: Epoch, peer: &Id) -> Vec<u8> {
    let mut buf = Encoder::new(Cursor::new(Vec::new()));
    id.encode(&mut buf).unwrap();
    buf.u64(epoch).unwrap();
    buf.u8(VOTE).unwrap();
    peer.encode(&mut buf).unwrap();
    return buf.into_writer().into_inner();
}

pub fn read_packet(buf: &[u8]) -> DecodeResult<Capsule> {
    let mut dec = Decoder::new(Config::default(), Cursor::new(buf));
    let source = try!(Id::decode(&mut dec));
    let epoch = try!(dec.u64());
    match try!(dec.u8()) {
        x if x == PING => Ok((source, epoch, Message::Ping)),
        x if x == VOTE => {
            let peer = try!(Id::decode(&mut dec));
            Ok((source, epoch, Message::Vote(peer)))
        }
        x => Err(DecodeError::UnexpectedType {
            datatype: Type::UInt32,
            info: x,
        }),
    }
}
