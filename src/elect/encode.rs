use std::io::Cursor;
use cbor::{Encoder, Decoder, Config, DecodeResult};
use cbor::{DecodeError};
use cbor::types::Type;

use shared::Id;
use super::{Capsule, Message};
use super::machine::Epoch;

const PING: u8 = 1;
const PONG: u8 = 2;
const VOTE: u8 = 3;


pub fn ping(id: &Id, epoch: Epoch, schedule_hash: &String) -> Vec<u8> {
    let mut buf = Encoder::new(Cursor::new(Vec::new()));
    id.encode(&mut buf).unwrap();
    buf.u64(epoch).unwrap();
    buf.u8(PING).unwrap();
    buf.text(schedule_hash).unwrap();
    return buf.into_writer().into_inner();
}

pub fn pong(id: &Id, epoch: Epoch) -> Vec<u8> {
    let mut buf = Encoder::new(Cursor::new(Vec::new()));
    id.encode(&mut buf).unwrap();
    buf.u64(epoch).unwrap();
    buf.u8(PONG).unwrap();
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
        x if x == PING => {
            let peer_id = try!(dec.text());
            Ok((source, epoch, Message::Ping { config_hash: peer_id }))
        }
        x if x == PONG => Ok((source, epoch, Message::Pong)),
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
