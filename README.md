# local-rolling-file

[![local-rolling-file on GitHub Actions](https://github.com/WANG-lp/local-rolling-file/actions/workflows/test.yaml/badge.svg)](https://github.com/WANG-lp/local-rolling-file/actions/workflows/test.yaml)
[![local-rolling-file on crates.io](https://img.shields.io/crates/v/local-rolling-file.svg)](https://crates.io/crates/local-rolling-file)
[![rolling-file on docs.rs](https://docs.rs/local-rolling-file/badge.svg)](https://docs.rs/local-rolling-file)
[![GitHub: WANG-lp/local-rolling-file](https://img.shields.io/badge/GitHub-WANG-lp%2Flocal--rolling--file--lightgrey?logo=github&style=flat-square)](https://github.com/WANG-lp/local-rolling-file)
![license: MIT or Apache-2.0](https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-red?style=flat-square)
![minimum rustc: 1.42](https://img.shields.io/badge/minimum%20rustc-1.42-yellowgreen?logo=rust&style=flat-square)


NOTE: this repo is based on https://github.com/Axcient/rolling-file-rs

A rolling file appender with customizable rolling conditions.
Includes built-in support for rolling conditions on date/time
(daily, hourly, every minute) and/or size.

Log files structures(with `log` as folder and `log.log` as prefix):
- log.log `(a symbol link always points to the latest one log file)`
- log.log.yyyymmdd.hhmmss `(e.g. log.log.20240520.010101)`
- ..

This is useful to combine with the [tracing](https://crates.io/crates/tracing) crate and
[tracing_appender::non_blocking::NonBlocking](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/index.html) -- use it
as an alternative to [tracing_appender::rolling::RollingFileAppender](https://docs.rs/tracing-appender/latest/tracing_appender/rolling/struct.RollingFileAppender.html).

## Examples

```rust
use rolling_file::*;
let file_appender = BasicRollingFileAppender::new(
    "./log", // folder
    "log.log", // prefix
    RollingConditionBasic::new().daily(),
    9
).unwrap();
```

```rust
let file_appender = local_rolling_file::RollingFileAppender::new(
        folder,
        "log.log",
        local_rolling_file::RollingConditionBasic::new().daily(),
        3,
    )
    .unwrap();
    let (log_file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let local_time = tracing_subscriber::fmt::time::OffsetTime::new(
        UtcOffset::from_hms(8, 0, 0).unwrap(),
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:6]"),
    );
    let subscriber = tracing_subscriber::Registry::default()
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(false)
                .with_file(true)
                .with_line_number(true)
                .with_ansi(false)
                .with_timer(local_time.clone())
                .with_writer(log_file_writer)
                .with_filter(level),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(false)
                .with_file(true)
                .with_line_number(true)
                .with_ansi(true)
                .with_timer(local_time)
                .with_filter(level),
        );

    // use that subscriber to process traces emitted after this point
    let _ = tracing::subscriber::set_global_default(subscriber);
    tracing::info!("Logger set up successfully");
```


## Development

Must pass latest stable clippy, be formatted with nightly rustfmt, and pass unit tests:

```
cargo +nightly fmt
cargo clippy --all-targets
cargo test
```

## License

Dual-licensed under the terms of either the MIT license or the Apache 2.0 license.

## Changelog

See [CHANGELOG.md](CHANGELOG.md)
