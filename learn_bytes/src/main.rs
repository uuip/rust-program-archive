use std::error::Error;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use chrono::Local;
use log::{debug, info};

use common::init_logger;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    init_logger();
    let now = Local::now();
    // num to bytes
    let mut buf = BytesMut::with_capacity(64);
    buf.put_i32(222);

    // to num from &[u8]
    let v = Bytes::from(buf.to_vec()).get_i32();
    info!("{v}");
    // with byteorder
    let v = buf.as_ref().read_i32::<BigEndian>()?;
    info!("{v}");

    // std only
    let sa = "aaa".as_bytes();
    let _ = String::from_utf8(Vec::from(sa))?;
    let sb = "bbb".as_bytes();
    let _ = String::from_utf8([sa, sb].concat())?;

    let str_bytes_a = Bytes::from("aaa");
    let str_bytes_b = Bytes::from("bbb");
    // let bytes_c=Bytes::from([str_bytes_a,str_bytes_b].concat());
    let mut bytes_c = BytesMut::with_capacity(1024);
    bytes_c.put(str_bytes_a);
    bytes_c.put(str_bytes_b);
    let _ = String::from_utf8(bytes_c.to_vec())?;

    info!(
        "run time: {}s",
        Local::now().signed_duration_since(now).num_seconds()
    );
    debug!("this id debug");
    Ok(())
}
