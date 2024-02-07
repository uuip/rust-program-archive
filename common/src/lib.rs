#[cfg(feature = "pg")]
pub use self::db::create_pool;
#[cfg(feature = "file-logger")]
pub use self::logger::init_file_logger;
pub use self::logger::init_logger;
#[cfg(feature = "tracing")]
pub use self::logger::init_tracing_logger;
pub use self::setting::Setting;

#[cfg(feature = "web3")]
pub mod erc20;
#[cfg(feature = "pg-with-model")]
pub mod model;
#[cfg(feature = "pulsar")]
pub mod schema;
mod setting;

#[cfg(feature = "pg")]
pub mod db {
    use std::str::FromStr;

    use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
    use tokio_postgres::NoTls;

    pub async fn create_pool(db_url: &str) -> Pool {
        let pg_config = tokio_postgres::Config::from_str(db_url).unwrap();
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(pg_config, NoTls, mgr_config);
        Pool::builder(mgr).max_size(100).build().unwrap()
    }
}

pub mod logger {
    use std::io::Write;

    use chrono::Local;
    use env_logger::fmt::style::Color;
    #[cfg(feature = "file-logger")]
    use flexi_logger::{
        colored_opt_format, opt_format, Cleanup, Criterion, Duplicate, FileSpec, FlexiLoggerError,
        LoggerHandle, Naming, WriteMode,
    };
    use log::{Level, LevelFilter};

    // RUST_LOG=error, ["OFF", "ERROR", "WARN", "INFO", "DEBUG", "TRACE"]
    // RUST_LOG=error,hello=warn
    pub fn init_logger() {
        env_logger::builder()
            .filter_level(LevelFilter::Debug)
            .format(|buf, record| {
                let mut level_style = buf.default_level_style(record.level());
                let reset = level_style.render_reset();
                if record.level() == Level::Warn {
                    level_style = level_style.fg_color(Some(Color::Ansi256(206_u8.into())));
                }
                let level_style = level_style.render();
                writeln!(
                    buf,
                    "{level_style}[{} | line:{:<4}|{}]: {}{reset}",
                    Local::now().format("%H:%M:%S"),
                    record.line().unwrap_or(0),
                    record.level(),
                    record.args()
                )
            })
            .init();
    }

    #[cfg(feature = "file-logger")]
    pub fn init_file_logger() -> Result<LoggerHandle, FlexiLoggerError> {
        // WriteMode::BufferAndFlush 需要主进程保持LoggerHandle存活, let _l = init_file_logger();
        // 不能使用 let _ = ...
        flexi_logger::Logger::try_with_env_or_str(LevelFilter::Info.as_str())?
            .log_to_file(
                FileSpec::default()
                    .basename("someservice")
                    .directory("./logs"),
            )
            .rotate(
                Criterion::Size(100_u64 * 1024_u64.pow(2_u32)),
                Naming::NumbersDirect,
                Cleanup::KeepLogFiles(7),
            )
            .append()
            .write_mode(WriteMode::BufferAndFlush)
            .duplicate_to_stdout(Duplicate::Info)
            .format_for_files(opt_format)
            .format_for_stdout(colored_opt_format)
            .start()
    }

    #[cfg(feature = "tracing")]
    pub fn init_tracing_logger() -> tracing_appender::non_blocking::WorkerGuard {
        use tracing_subscriber::fmt::writer::MakeWriterExt;
        use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

        // use tracing_subscriber::fmt::time::OffsetTime;
        // use time::macros::format_description;
        // let offset = time::UtcOffset::current_local_offset().expect("should get local offset!");
        // let timer = OffsetTime::new(
        //     offset,
        //     format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"));

        use tracing_subscriber::fmt::time::ChronoLocal;
        let timer = ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f%:z".to_string());

        // tracing_appender当前版本未实现按文件大小切割

        // use tracing_appender::rolling::Rotation;
        // let file_appender = tracing_appender::rolling::Builder::new()
        //     .filename_prefix("thisapp")
        //     .max_log_files(10)
        //     .rotation(Rotation::DAILY)
        //     .build(".").unwrap();

        // tracing_rolling_file 支持文件大小
        use tracing_rolling_file::{RollingConditionBase, RollingFileAppender};
        let file_appender = RollingFileAppender::new(
            "./thisapp",
            RollingConditionBase::new().max_size(100_u64 * 1024_u64.pow(2_u32)),
            10,
        )
        .unwrap();

        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        let file_layer = fmt::Layer::new()
            .with_timer(timer.clone())
            .with_ansi(false)
            .with_writer(non_blocking.with_max_level(tracing::Level::WARN));
        let console_layer = fmt::Layer::new().with_timer(timer);
        tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer)
            .init();
        _guard
    }
}
