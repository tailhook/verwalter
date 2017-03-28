use std::io;
use std::io::Read;
use std::fs::File;

use nix::unistd::gethostname;
use rustc_serialize::hex::FromHex;

use id::Id;


pub fn machine_id() -> io::Result<Id> {
    let mut buf = String::with_capacity(33);
    let mut file = try!(File::open("/etc/machine-id"));
    let bytes = try!(file.read_to_string(&mut buf));
    if bytes != 32 && bytes != 33 {
        return Err(io::Error::new(io::ErrorKind::Other,
            "Wrong length of /etc/machine-id"));
    }
    let bin = try!(FromHex::from_hex(&buf[..])
        .or_else(|_| Err(io::Error::new(io::ErrorKind::Other,
            "Error decoding /etc/machine-id. Should be hexadecimal."))));
    Ok(Id::new(bin))
}

pub fn hostname() -> Result<String, String> {
    let mut buf = [0u8; 255];
    try!(gethostname(&mut buf)
        .map_err(|e| format!("Can't get hostname: {:?}", e)));
    buf.iter().position(|&x| x == 0)
        .ok_or(format!("Hostname is not terminated"))
        .and_then(|idx| String::from_utf8(buf[..idx].to_owned())
            .map_err(|e| format!("Can't decode hostname: {}", e)))
}
