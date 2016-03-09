use std::io::Read;
use std::fs::File;
use std::net::{SocketAddr};
use std::ptr::{null_mut};

use nix;
use nix::errno::Errno;
use nix::unistd::gethostname;
use nix::sys::socket::{InetAddr, sockaddr_in};
use libc::{getifaddrs, freeifaddrs};
use libc::{AF_INET};
use rustc_serialize::hex::FromHex;
use rand;
use rand::Rng;

use elect::Id;


pub fn machine_id() -> Id {
    let mut buf = String::with_capacity(33);
    let bytes = File::open("/etc/machine-id")
    .and_then(|mut f| f.read_to_string(&mut buf))
    .map_err(|e| error!("Error reading /etc/machine-id: {}", e))
    .and_then(|bytes| if bytes != 32 && bytes != 33  {
        error!("Wrong length of /etc/machine-id");
        Err(())
    } else {
        FromHex::from_hex(&buf[..])
        .map_err(|e| error!("Error decoding /etc/machine-id: {}", e))
    }).unwrap_or_else(|_| {
        panic!("The file `/etc/machine-id` is mandatory");
    });
    Id::new(bytes)
}

pub fn hostname() -> Result<String, String> {
    let mut buf = [0u8; 255];
    try!(gethostname(&mut buf)
        .map_err(|e| format!("gethostname: Can't get hostname: {:?}", e)));
    buf.iter().position(|&x| x == 0)
        .ok_or(format!("gethostname: Hostname is not terminated"))
        .and_then(|idx| String::from_utf8(buf[..idx].to_owned())
            .map_err(|e| format!("Can't decode hostname: {}", e)))
}
