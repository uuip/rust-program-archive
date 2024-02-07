#![allow(unused_imports)]

use std::str::FromStr;

use chrono::Local;
use ethers::abi::AbiDecode;
use ethers::prelude::Bytes;
use futures_util::{pin_mut, StreamExt};
use log::info;
use once_cell::sync::Lazy;
use pulsar::{ProducerOptions, Pulsar, TokioExecutor};
use tokio::join;
use tokio_postgres::types::ToSql;
use tokio_postgres::Row;

use common::erc20::Erc20TokenCalls;
use common::schema::{Msg, PulsarSchema, TokenMessageArg};
use common::{create_pool, init_logger, Setting};

static SETTING: Lazy<Setting, fn() -> Setting> = Lazy::new(Setting::init);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();
    let (ps, pr) = async_channel::unbounded::<Msg>();
    let pool = create_pool(&SETTING.explorer_db).await;
    let now = Local::now();

    let pulsar: Pulsar<TokioExecutor> = Pulsar::builder(&SETTING.pulsar_addr, TokioExecutor)
        .build()
        .await?;
    let mut producer = pulsar
        .producer()
        .with_topic(&SETTING.topic)
        .with_options(ProducerOptions {
            // compression: Some(Compression::Lz4(CompressionLz4::default())),
            schema: Some(Msg::pulsar_json_schema()),
            batch_size: Some(1000),
            ..Default::default()
        })
        .build()
        .await?;

    let pulsar_task = tokio::spawn(async move {
        while let Ok(msg) = pr.recv().await {
            producer.send(msg).await?;
        }
        info!("pulsar_task exit");
        let _ = producer.send_batch().await;
        let _ = producer.close().await;
        Ok::<(), anyhow::Error>(())
    });

    let params: [&(dyn ToSql + Sync); 0] = [];
    let conn = pool.get().await?;
    let rows = conn
        .query_raw(
            "SELECT id, tx_str FROM transaction_history_1 ORDER BY id limit 100;",
            params,
        )
        .await?;
    pin_mut!(rows);
    while let Some(Ok(row)) = rows.next().await {
        let ps = ps.clone();
        tokio::spawn(async move {
            let msg = process(row);
            if let Ok(msg) = msg {
                let _ = ps.send(msg).await;
            };
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

fn process(row: Row) -> Result<Msg, anyhow::Error> {
    let input: String = row.get("tx_str");
    let b_input = Bytes::from_str(&input)?;
    let decode_input = Erc20TokenCalls::decode(b_input)?;
    if let Erc20TokenCalls::TokenTransfer(v) = decode_input {
        let args: TokenMessageArg = serde_json::from_str(&v.message)?;
        let msg = args.make_msg_with_ext(v.message);
        Ok(msg)
    } else {
        Err(anyhow::anyhow!("parsed error"))
    }
}
