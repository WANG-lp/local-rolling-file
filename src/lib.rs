//! A rolling file appender with customizable rolling conditions.
//! Includes built-in support for rolling conditions on date/time
//! (daily, hourly, every minute) and/or size.

//! Log files structures(with `log` as folder and `log.log` as prefix):
//! - log.log `(a symbol link always points to the latest one log file)`
//! - log.log.yyyymmdd.hhmmss `(e.g. log.log.20240520.010101)`
//! - ..

//! This is useful to combine with the tracing crate and
//! tracing_appender::non_blocking::NonBlocking -- use it
//! as an alternative to tracing_appender::rolling::RollingFileAppender.
//!
//! # Examples
//!
//! ```rust
//! # fn docs() {
//! # use rolling_file::*;
//! let file_appender = BasicRollingFileAppender::new(
//!     "./log",
//!     "log.log",
//!     RollingConditionBasic::new().daily(),
//!     9
//! ).unwrap();
//! # }
//! ```
#![deny(warnings)]

use chrono::prelude::*;
use std::{
    convert::TryFrom,
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::Path,
};
use symlink::{remove_symlink_auto, symlink_auto};

/// Determines when a file should be "rolled over".
pub trait RollingCondition {
    /// Determine and return whether or not the file should be rolled over.
    fn should_rollover(&mut self, now: &DateTime<Local>, current_filesize: u64) -> bool;
}

/// Determines how often a file should be rolled over
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RollingFrequency {
    EveryDay,
    EveryHour,
    EveryMinute,
}

impl RollingFrequency {
    /// Calculates a datetime that will be different if data should be in
    /// different files.
    pub fn equivalent_datetime(&self, dt: &DateTime<Local>) -> DateTime<Local> {
        match self {
            RollingFrequency::EveryDay => Local
                .with_ymd_and_hms(dt.year(), dt.month(), dt.day(), 0, 0, 0)
                .unwrap(),
            RollingFrequency::EveryHour => Local
                .with_ymd_and_hms(dt.year(), dt.month(), dt.day(), dt.hour(), 0, 0)
                .unwrap(),
            RollingFrequency::EveryMinute => Local
                .with_ymd_and_hms(dt.year(), dt.month(), dt.day(), dt.hour(), dt.minute(), 0)
                .unwrap(),
        }
    }
}

/// Implements a rolling condition based on a certain frequency
/// and/or a size limit. The default condition is to rotate daily.
///
/// # Examples
///
/// ```rust
/// use rolling_file::*;
/// let c = RollingConditionBasic::new().daily();
/// let c = RollingConditionBasic::new().hourly().max_size(1024 * 1024);
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RollingConditionBasic {
    last_write_opt: Option<DateTime<Local>>,
    frequency_opt: Option<RollingFrequency>,
    max_size_opt: Option<u64>,
}

impl RollingConditionBasic {
    /// Constructs a new struct that does not yet have any condition set.
    pub fn new() -> RollingConditionBasic {
        RollingConditionBasic {
            last_write_opt: None,
            frequency_opt: None,
            max_size_opt: None,
        }
    }

    /// Sets a condition to rollover on the given frequency
    pub fn frequency(mut self, x: RollingFrequency) -> RollingConditionBasic {
        self.frequency_opt = Some(x);
        self
    }

    /// Sets a condition to rollover when the date changes
    pub fn daily(mut self) -> RollingConditionBasic {
        self.frequency_opt = Some(RollingFrequency::EveryDay);
        self
    }

    /// Sets a condition to rollover when the date or hour changes
    pub fn hourly(mut self) -> RollingConditionBasic {
        self.frequency_opt = Some(RollingFrequency::EveryHour);
        self
    }

    pub fn minutely(mut self) -> RollingConditionBasic {
        self.frequency_opt = Some(RollingFrequency::EveryMinute);
        self
    }

    /// Sets a condition to rollover when a certain size is reached
    pub fn max_size(mut self, x: u64) -> RollingConditionBasic {
        self.max_size_opt = Some(x);
        self
    }
}

impl Default for RollingConditionBasic {
    fn default() -> Self {
        RollingConditionBasic::new().frequency(RollingFrequency::EveryDay)
    }
}

impl RollingCondition for RollingConditionBasic {
    fn should_rollover(&mut self, now: &DateTime<Local>, current_filesize: u64) -> bool {
        let mut rollover = false;
        if let Some(frequency) = self.frequency_opt.as_ref() {
            if let Some(last_write) = self.last_write_opt.as_ref() {
                if frequency.equivalent_datetime(now) != frequency.equivalent_datetime(last_write) {
                    rollover = true;
                }
            }
        }
        if let Some(max_size) = self.max_size_opt.as_ref() {
            if current_filesize >= *max_size {
                rollover = true;
            }
        }
        self.last_write_opt = Some(*now);
        rollover
    }
}

/// Writes data to a file, and "rolls over" to preserve older data in
/// a separate set of files. Old files have a Debian-style naming scheme
/// where we have base_filename, base_filename.1, ..., base_filename.N
/// where N is the maximum number of rollover files to keep.
#[derive(Debug)]
pub struct RollingFileAppender<RC>
where
    RC: RollingCondition,
{
    condition: RC,
    folder: String,
    prefix: String,
    max_files: usize,
    buffer_capacity: Option<usize>,
    current_filesize: u64,
    writer_opt: Option<BufWriter<File>>,
}

impl<RC> RollingFileAppender<RC>
where
    RC: RollingCondition,
{
    /// Creates a new rolling file appender with the given condition.
    /// The parent directory of the base path must already exist.
    pub fn new(folder: &str, prefix: &str, condition: RC, max_files: usize) -> io::Result<RollingFileAppender<RC>> {
        Self::_new(folder, prefix, condition, max_files, None)
    }

    /// Creates a new rolling file appender with the given condition and write buffer capacity.
    /// The parent directory of the base path must already exist.
    pub fn new_with_buffer_capacity(
        folder: &str,
        prefix: &str,
        condition: RC,
        max_files: usize,
        buffer_capacity: usize,
    ) -> io::Result<RollingFileAppender<RC>> {
        Self::_new(folder, prefix, condition, max_files, Some(buffer_capacity))
    }

    fn _new(
        folder: &str,
        prefix: &str,
        condition: RC,
        max_files: usize,
        buffer_capacity: Option<usize>,
    ) -> io::Result<RollingFileAppender<RC>> {
        let folder = folder.to_string();
        let prefix = prefix.to_string();
        let mut rfa = RollingFileAppender {
            condition,
            folder,
            prefix,
            max_files,
            buffer_capacity,
            current_filesize: 0,
            writer_opt: None,
        };
        // Fail if we can't open the file initially...
        rfa.open_writer_if_needed(&Local::now())?;
        Ok(rfa)
    }

    fn check_and_remove_log_file(&mut self) -> io::Result<()> {
        let files = std::fs::read_dir(&self.folder)?;

        let mut log_files = vec![];
        for f in files {
            if let Ok(f) = f {
                let fname = f.file_name().to_string_lossy().to_string();
                if fname.starts_with(&self.prefix) && !fname.ends_with(".latest") {
                    log_files.push(fname);
                }
            }
        }

        log_files.sort_by(|a, b| b.cmp(a));

        if log_files.len() > self.max_files {
            for f in log_files.drain(self.max_files..) {
                let p = Path::new(&self.folder).join(f);
                if let Err(e) = fs::remove_file(&p) {
                    tracing::error!("WARNING: Failed to remove old logfile {}: {}", p.to_string_lossy(), e);
                }
            }
        }
        Ok(())
    }

    /// Forces a rollover to happen immediately.
    pub fn rollover(&mut self, now: &DateTime<Local>) -> io::Result<()> {
        // Before closing, make sure all data is flushed successfully.
        self.flush()?;
        // We must close the current file before rotating files
        self.writer_opt.take();
        self.current_filesize = 0;
        self.open_writer_if_needed(now)
    }

    /// Returns a reference to the rolling condition
    pub fn condition_ref(&self) -> &RC {
        &self.condition
    }

    /// Returns a mutable reference to the rolling condition, possibly to mutate its state dynamically.
    pub fn condition_mut(&mut self) -> &mut RC {
        &mut self.condition
    }

    fn new_file_name(&self, now: &DateTime<Local>) -> String {
        let data_str = now.format("%Y%m%d.%H%M%S").to_string();
        format!("{}.{}", self.prefix, data_str)
    }

    /// Opens a writer for the current file.
    fn open_writer_if_needed(&mut self, now: &DateTime<Local>) -> io::Result<()> {
        if self.writer_opt.is_none() {
            let p = self.new_file_name(now);
            let new_file_path = std::path::Path::new(&self.folder).join(&p);
            let f = OpenOptions::new().append(true).create(true).open(&new_file_path)?;
            self.writer_opt = Some(if let Some(capacity) = self.buffer_capacity {
                BufWriter::with_capacity(capacity, f)
            } else {
                BufWriter::new(f)
            });
            // make a soft link to latest file
            {
                let folder = std::path::Path::new(&self.folder);
                if let Ok(path) = folder.canonicalize() {
                    let latest_log_symlink = path.join(&self.prefix);
                    let _ = remove_symlink_auto(folder.join(&self.prefix));
                    let _ = symlink_auto(&new_file_path.canonicalize().unwrap(), &latest_log_symlink);
                }
            }
            self.current_filesize = fs::metadata(&p).map_or(0, |m| m.len());
            self.check_and_remove_log_file()?;
        }
        Ok(())
    }

    /// Writes data using the given datetime to calculate the rolling condition
    pub fn write_with_datetime(&mut self, buf: &[u8], now: &DateTime<Local>) -> io::Result<usize> {
        if self.condition.should_rollover(now, self.current_filesize) {
            if let Err(e) = self.rollover(now) {
                // If we can't rollover, just try to continue writing anyway
                // (better than missing data).
                // This will likely used to implement logging, so
                // avoid using log::warn and log to stderr directly
                eprintln!("WARNING: Failed to rotate logfile  {}", e);
            }
        }
        self.open_writer_if_needed(now)?;
        if let Some(writer) = self.writer_opt.as_mut() {
            let buf_len = buf.len();
            writer.write_all(buf).map(|_| {
                self.current_filesize += u64::try_from(buf_len).unwrap_or(u64::MAX);
                buf_len
            })
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "unexpected condition: writer is missing",
            ))
        }
    }
}

impl<RC> io::Write for RollingFileAppender<RC>
where
    RC: RollingCondition,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let now = Local::now();
        self.write_with_datetime(buf, &now)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(writer) = self.writer_opt.as_mut() {
            writer.flush()?;
        }
        Ok(())
    }
}

/// A rolling file appender with a rolling condition based on date/time or size.
pub type BasicRollingFileAppender = RollingFileAppender<RollingConditionBasic>;

#[cfg(test)]
mod t {
    #[test]
    fn test_number_of_log_files() {
        use super::*;
        let folder = "./log";
        let prefix = "log.log";

        let _ = std::fs::remove_dir_all(folder);
        std::fs::create_dir(folder).unwrap();

        let condition = RollingConditionBasic::new().hourly();
        let max_files = 3;
        let mut rfa = RollingFileAppender::new(folder, prefix, condition, max_files).unwrap();
        rfa.write_with_datetime(b"Line 1\n", &Local.with_ymd_and_hms(2021, 3, 30, 1, 2, 3).unwrap())
            .unwrap();
        rfa.write_with_datetime(b"Line 2\n", &Local.with_ymd_and_hms(2021, 3, 30, 2, 3, 0).unwrap())
            .unwrap();
        rfa.write_with_datetime(b"Line 3\n", &Local.with_ymd_and_hms(2021, 3, 31, 1, 4, 0).unwrap())
            .unwrap();
        rfa.write_with_datetime(b"Line 4\n", &Local.with_ymd_and_hms(2021, 5, 31, 1, 4, 0).unwrap())
            .unwrap();
        rfa.write_with_datetime(b"Line 5\n", &Local.with_ymd_and_hms(2022, 5, 31, 2, 4, 0).unwrap())
            .unwrap();
        rfa.flush().unwrap();
        let files = std::fs::read_dir(folder).unwrap();
        let mut log_files = vec![];
        for f in files {
            if let Ok(f) = f {
                let fname = f.file_name().to_string_lossy().to_string();
                if fname.starts_with(prefix) && !fname.ends_with(".latest") {
                    log_files.push(fname);
                }
            }
        }
        assert_eq!(log_files.len(), max_files);
    }
}
