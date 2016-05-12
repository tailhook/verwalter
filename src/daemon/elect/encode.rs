use std::io::{Cursor, Write};
use std::sync::Arc;

use cbor::{Encoder, Decoder, Config, DecodeResult};
use cbor::{DecodeError, EncodeResult, opt};
use cbor::types::Type;

use shared::{Id};
use super::{Capsule, Message, ScheduleStamp};
use super::machine::Epoch;
use scheduler::Schedule;

const PING: u8 = 1;
const PONG: u8 = 2;
const VOTE: u8 = 3;

fn write_metadata<W: Write>(enc: &mut Encoder<W>,
    schedule: &Option<Arc<Schedule>>)
    -> EncodeResult
{
    if let &Some(ref schedule) = schedule {
        try!(enc.array(4));
        try!(enc.text("schedule"));
        try!(enc.u64(schedule.timestamp));
        try!(enc.text(&schedule.hash));
        try!(schedule.origin.encode_cbor(enc));
    } else {
        try!(enc.null());
    }
    Ok(())
}

pub fn ping(id: &Id, epoch: Epoch, schedule: &Option<Arc<Schedule>>)
    -> Vec<u8>
{
    let mut buf = Encoder::new(Cursor::new(Vec::new()));
    id.encode_cbor(&mut buf).unwrap();
    buf.u64(epoch).unwrap();
    buf.u8(PING).unwrap();
    write_metadata(&mut buf, schedule).unwrap();
    return buf.into_writer().into_inner();
}

pub fn pong(id: &Id, epoch: Epoch, schedule: &Option<Arc<Schedule>>) -> Vec<u8> {
    let mut buf = Encoder::new(Cursor::new(Vec::new()));
    id.encode_cbor(&mut buf).unwrap();
    buf.u64(epoch).unwrap();
    buf.u8(PONG).unwrap();
    write_metadata(&mut buf, schedule).unwrap();
    return buf.into_writer().into_inner();
}

pub fn vote(id: &Id, epoch: Epoch, peer: &Id, schedule: &Option<Arc<Schedule>>)
    -> Vec<u8>
{
    let mut buf = Encoder::new(Cursor::new(Vec::new()));
    id.encode_cbor(&mut buf).unwrap();
    buf.u64(epoch).unwrap();
    buf.u8(VOTE).unwrap();
    peer.encode_cbor(&mut buf).unwrap();
    write_metadata(&mut buf, schedule).unwrap();
    return buf.into_writer().into_inner();
}

pub fn read_packet(buf: &[u8]) -> DecodeResult<Capsule> {
    let mut dec = Decoder::new(Config::default(), Cursor::new(buf));
    let source = try!(Id::decode(&mut dec));
    let epoch = try!(dec.u64());
    let (src, epoch, msg)  = match try!(dec.u8()) {
        x if x == PING => (source, epoch, Message::Ping),
        x if x == PONG => (source, epoch, Message::Pong),
        x if x == VOTE => {
            let peer = try!(Id::decode(&mut dec));
            (source, epoch, Message::Vote(peer))
        }
        x => return Err(DecodeError::UnexpectedType {
            datatype: Type::UInt32,
            info: x,
        }),
    };
    let sstamp = match try!(opt(dec.array())) {
        Some(4) => {
            match try!(dec.text_borrow()) {
                "schedule" => {
                    let tstamp = try!(dec.u64());
                    let hash = try!(dec.text());
                    let origin = try!(Id::decode(&mut dec));
                    Some(ScheduleStamp {
                        timestamp: tstamp,
                        hash: hash,
                        origin: origin,
                    })
                }
                _ => {
                    None
                }
            }
        }
        _ => None,
    };
    Ok(Capsule {
        source: src,
        epoch: epoch,
        message: msg,
        schedule: sstamp,
    })
}
