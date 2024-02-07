use std::str::FromStr;
use std::sync::Arc;

use chrono::Local;
use ethers::abi::AbiDecode;
use ethers::prelude::Bytes;
use futures_util::{pin_mut, StreamExt};
use log::{error, info};
use once_cell::sync::Lazy;
use pulsar::{ProducerOptions, Pulsar, TokioExecutor};
use tokio::join;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{mpsc, Semaphore};
use tokio_postgres::Row;

use common::erc20::Erc20TokenCalls;
use common::schema::{Msg, PulsarSchema, TokenMessageArg};
use common::{create_pool, init_logger, Setting};

static SETTING: Lazy<Setting, fn() -> Setting> = Lazy::new(Setting::init);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();

    let pulsar: Pulsar<TokioExecutor> = Pulsar::builder(&SETTING.pulsar_addr, TokioExecutor)
        .build()
        .await?;
    let mut producer = pulsar
        .producer()
        .with_topic(&SETTING.topic)
        .with_options(ProducerOptions {
            schema: Some(Msg::pulsar_json_schema()),
            batch_size: Some(1000),
            ..Default::default()
        })
        .build()
        .await?;

    let semaphore = Arc::new(Semaphore::new(5000));
    let (ps, mut pr) = mpsc::unbounded_channel::<Msg>();

    let pulsar_task = tokio::spawn(async move {
        while let Some(msg) = pr.recv().await {
            producer.send(msg).await?;
        }
        let _ = producer.send_batch().await;
        let _ = producer.close().await;
        Ok::<(), anyhow::Error>(())
    });
    let pool = create_pool(&SETTING.explorer_db).await;
    let conn = pool.get().await?;

    let now = Local::now();
    let params: [String; 0] = [];
    let rows = conn
        .query_raw(
            "SELECT id, tx_str FROM transaction_history_1 ORDER BY id limit 500000",
            params,
        )
        .await?;
    pin_mut!(rows);
    while let Some(Ok(row)) = rows.next().await {
        let s = ps.clone();
        let semaphore = semaphore.clone();
        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let _ = process(row, s).await;
        });
    }
    drop(ps);
    let _ = join!(pulsar_task);
    info!(
        "run time: {}s",
        Local::now().signed_duration_since(now).num_seconds()
    );
    Ok(())
}

async fn process(row: Row, s: UnboundedSender<Msg>) -> Result<(), anyhow::Error> {
    let id: i64 = row.get("id");
    // info!("process {}",id);
    let input: String = row.get("tx_str");
    let b_input = Bytes::from_str(&input)?;
    let decode_input = Erc20TokenCalls::decode(b_input)?;
    if let Erc20TokenCalls::TokenTransfer(v) = decode_input {
        let args: TokenMessageArg = serde_json::from_str(&v.message)?;
        let msg = args.make_msg_with_ext(v.message);
        s.send(msg).unwrap_or_else(|e| error!("{e} {id}"));
    };
    // sleep(std::time::Duration::from_secs(10)).await;
    Ok(())
}
