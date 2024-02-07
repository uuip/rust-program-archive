use std::cmp::min;
use std::sync::Arc;
use std::time::Duration;

use chrono::prelude::*;
use chrono_tz::Asia::Tokyo;
use ethers::prelude::*;
use futures_util::TryStreamExt;
use log::{error, info, warn};
use rand::prelude::{IteratorRandom, SeedableRng, StdRng};
use serde_json::Value;
use sqlx::prelude::*;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqliteConnection;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use common::erc20::*;
use common::{init_logger, Setting};

use crate::schema::Msg;
use crate::utils::CustomRetryPolicy;

mod schema;
mod utils;

static SETTING: Lazy<Setting, fn() -> Setting> = Lazy::new(Setting::init);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();
    let token_addr = "0xce634F8225f3276183FdCace7B97C817eAABaaaD";
    let jp_log_date = "2023-11-28";
    let step = 100_u64;
    let batch_size = SETTING.rpc_list.len() * 4;

    let t1 = Local::now();
    let mut tasks = JoinSet::new();
    let (s, r) = async_channel::bounded(batch_size);
    let (dbs, mut dbr) = mpsc::unbounded_channel();

    let s_produce = s.clone();
    let s_task = tokio::spawn(async move {
        let mut rng = StdRng::from_entropy();
        let host = reqwest::Url::parse(SETTING.rpc_list.iter().choose(&mut rng).unwrap())?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .tcp_keepalive(Duration::from_secs(300))
            .build()?;
        let provider = Http::new_with_client(host, client);
        let w3: Arc<Provider<Http>> = Arc::new(Provider::new(provider));
        // let w3: Arc<Provider<Http>> = Arc::new(Provider::<Http>::try_from(host)?);
        let current_block = w3.get_block(BlockNumber::Latest).await?.unwrap();
        let tx_date = jp_log_date.parse::<NaiveDate>()?;
        let tx_datetime = Tokyo
            .with_ymd_and_hms(tx_date.year(), tx_date.month(), tx_date.day(), 0, 0, 0)
            .unwrap();
        let from_block = current_block.number.unwrap().as_u64()
            - (current_block.timestamp.as_u64()
                - (tx_datetime - chrono::Duration::hours(1)).timestamp() as u64)
                / 3;
        let to_block = current_block.number.unwrap().as_u64()
            - (current_block.timestamp.as_u64()
                - (tx_datetime + chrono::Duration::hours(27)).timestamp() as u64)
                / 3;
        info!("from_block={}, to_block={}", from_block, to_block);

        for start in (from_block..=to_block).step_by(step as usize) {
            let stop = min(start + step - 1, to_block);
            s_produce.send(Msg { start, stop }).await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    for i in 0..batch_size {
        let r = r.clone();
        let dbs = dbs.clone();
        tasks.spawn(async move {
            let host = reqwest::Url::parse(&SETTING.rpc_list[i % SETTING.rpc_list.len()])?;
            let client = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(120))
                .tcp_keepalive(Duration::from_secs(300))
                .gzip(true)
                .build()?;
            let provider = Http::new_with_client(host, client);
            let retry_client = RetryClientBuilder::default()
                .rate_limit_retries(2)
                .build(provider, Box::new(CustomRetryPolicy));
            let w3 = Arc::new(Provider::new(retry_client));
            let c = Erc20Token::new(token_addr.parse::<Address>()?, w3.clone());
            while let Ok(msg) = r.recv().await {
                info!("worker {i}, get_started={}, stop={}", msg.start, msg.stop);
                let event = c.event::<TokenTransferFilter>();
                let rsp = event
                    .from_block(msg.start)
                    .to_block(msg.stop)
                    .query_with_meta()
                    .await;
                match rsp {
                    Ok(logs) => {
                        for (decoded_log, meta) in logs {
                            let message: Value = serde_json::from_str(&decoded_log.message)?;
                            // if let Some(gen_time) = message.get("gen_time") {
                            //     let str_gen_time =
                            //         gen_time.as_str().map(String::from).unwrap_or_default();
                            //     if &str_gen_time[..10] != jp_log_date {
                            //         // warn!("get different tx time {str_gen_time}");
                            //         // continue;
                            //     }
                            // }
                            let tag_id = message["tag_id"].as_str().map(String::from);
                            let log_time = message["gen_time"].as_str().map(String::from);
                            let block = meta.block_number.as_u64();
                            let tx_hash = format!("{:?}", meta.transaction_hash);
                            dbs.send((tag_id, tx_hash, block, log_time))?;
                        }
                    }
                    Err(e) => {
                        error!(
                            "worker {i} error:{} {e}, get_started={}, stop={}",
                            e.is_middleware_error(),
                            msg.start,
                            msg.stop
                        );
                    }
                }
            }
            warn!("worker {i} exited");
            Ok::<(), anyhow::Error>(())
        });
    }
    let dbtask = tokio::spawn(async move {
        let options = SqliteConnectOptions::new()
            .filename("txs.db")
            .create_if_missing(true);
        let mut db = SqliteConnection::connect_with(&options).await?;

        sqlx::query("CREATE TABLE IF NOT EXISTS transactions(id INTEGER PRIMARY KEY AUTOINCREMENT,tag_id text NOT NULL,hash text NOT NULL,block integer NOT NULL,log_time text)").execute(&mut db).await?;
        let mut tx = sqlx::Connection::begin(&mut db).await?;
        let stmt = tx
            .prepare("INSERT INTO transactions(tag_id,hash,block,log_time) VALUES(?,?,?,?)")
            .await?;
        while let Some(msg) = dbr.recv().await {
            stmt.query()
                .bind(msg.0)
                .bind(msg.1)
                .bind(msg.2 as i64)
                .bind(msg.3)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        let count: (i32, i32) =
            sqlx::query_as("SELECT COUNT(tag_id),COUNT(DISTINCT tag_id) FROM transactions")
                .fetch_one(&mut db)
                .await?;
        warn!("total tx_hash {}, distinct tx_hash {}", count.0, count.1);
        let mut rows = sqlx::query(
            "SELECT tag_id, COUNT(*) AS quantity FROM transactions GROUP BY tag_id HAVING quantity>1 limit 5",
        ).fetch(&mut db);
        while let Some(row) = rows.try_next().await? {
            warn!(
                "{},{}",
                row.get::<String, _>("tag_id"),
                row.get::<i32, _>("quantity")
            )
        }
        Ok::<(), anyhow::Error>(())
    });
    drop(dbs);
    drop(s);
    let _ = tokio::join!(s_task);
    while tasks.join_next().await.is_some() {}
    dbtask.await.unwrap()?;
    info!("run time {} s", (Local::now() - t1).num_seconds());
    Ok(())
}

pub trait AnyExt {
    fn type_name(&self) -> &'static str;
}

impl<T> AnyExt for T {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}
