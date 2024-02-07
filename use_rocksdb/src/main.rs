use std::thread::sleep;
use std::time::Duration;

use rocksdb::{DBCompactionStyle, DBCompressionType, Options, DB};
use serde_json::json;
use uuid::Uuid;

fn main() {
    let mut options = Options::default();
    options.create_if_missing(true);
    options.set_compaction_style(DBCompactionStyle::Level);
    options.set_compression_type(DBCompressionType::Lz4);
    options.set_level_compaction_dynamic_level_bytes(true);
    options.set_num_levels(4);
    options.set_bytes_per_sync(1048576);
    options.set_write_buffer_size(67108864);
    options.set_target_file_size_base(67108864);
    options.set_max_bytes_for_level_base(536870912);
    options.set_max_bytes_for_level_multiplier(8_f64);
    options.set_max_write_buffer_number(3);
    options.set_max_background_jobs(4);
    options.set_level_zero_file_num_compaction_trigger(8);
    options.set_level_zero_slowdown_writes_trigger(17);
    options.set_level_zero_stop_writes_trigger(24);
    let db = DB::open(&options, "./kvstore").unwrap();

    for _ in 0..1_0000_i32 {
        let v = json!({"block":fastrand::i32(..)});
        let v_bytes = serde_json::to_vec(&v).unwrap();
        db.put(Uuid::new_v4(), v_bytes).unwrap();
    }

    // if let Ok(Some(v)) = db.get("s".repeat(10)) {
    //     let rst = serde_json::from_slice::<HashMap<&str, i32>>(&v).unwrap();
    //     println!("{:?}", rst["aaa"]);
    // }

    sleep(Duration::from_secs(60));
}
