use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;

use rocksdb::{IteratorMode, Options, DB};
use uuid::{Bytes, Uuid};

fn main() {
    let options = Options::default();
    let db = DB::open_for_read_only(
        &options,
        "/Users/sharp/Desktop/rust/use_rocksdb/kvstore",
        false,
    )
    .unwrap();

    for (index, item) in db.iterator(IteratorMode::Start).flatten().enumerate() {
        if index > 9 {
            break;
        }
        let (key, value) = item;
        let key = Uuid::from_bytes(Bytes::try_from(key.to_vec()).unwrap());
        let value = serde_json::from_slice::<HashMap<&str, i32>>(&value).unwrap();
        println!("{}:{:?}", key, value);
    }
    sleep(Duration::from_secs(60));
}
