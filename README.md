# rolling-file-opt

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
