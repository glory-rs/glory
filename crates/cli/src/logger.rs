use ansi_term::Colour::Fixed;
use flexi_logger::{
    DeferredNow, Level, Record,
    filter::{LogLineFilter, LogLineWriter},
};
use once_cell::sync::Lazy;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use crate::ext::anyhow::Context;
use crate::{config::Log, ext::StrAdditions};

// https://gist.github.com/fnky/458719343aabd01cfb17a3a4f7296797
static ERR_RED: Lazy<ansi_term::Color> = Lazy::new(|| Fixed(196));
static WARN_YELLOW: Lazy<ansi_term::Color> = Lazy::new(|| Fixed(214));
pub static INFO_GREEN: Lazy<ansi_term::Color> = Lazy::new(|| Fixed(77));
static DBG_BLUE: Lazy<ansi_term::Color> = Lazy::new(|| Fixed(26));
static TRACE_VIOLET: Lazy<ansi_term::Color> = Lazy::new(|| Fixed(98));

pub static GRAY: Lazy<ansi_term::Color> = Lazy::new(|| Fixed(241));
// pub static BOLD: Lazy<ansi_term::Style> = Lazy::new(|| Style::new().bold());
static LOG_SELECT: Lazy<OnceLock<LogFlag>> = Lazy::new(OnceLock::new);
static LOGGER_RUNTIME: Lazy<OnceLock<LoggerRuntime>> = Lazy::new(OnceLock::new);

pub fn setup(verbose: u8, logs: &[Log]) {
    // OnceLock::get_or_try_init() is more idiomatic, but unstable at the moment
    _ = LOGGER_RUNTIME.get_or_init(|| {
        let handle = flexi_logger::Logger::try_with_str(log_level(verbose))
            .with_context(|| "Logger setup failed")
            .unwrap()
            .filter(Box::new(Filter))
            .format(format)
            .start()
            .unwrap();

        LoggerRuntime {
            handle,
            verbose: Mutex::new(normalize_verbose(verbose)),
        }
    });
    _ = LOG_SELECT.get_or_init(|| LogFlag::new(logs));
}

pub fn toggle_verbose() -> &'static str {
    let Some(runtime) = LOGGER_RUNTIME.get() else {
        return "info";
    };
    let mut verbose = runtime.verbose.lock().expect("logger verbosity lock poisoned");
    *verbose = next_verbose(*verbose);
    let level = log_level(*verbose);
    if let Err(err) = runtime.handle.parse_new_spec(level) {
        log::warn!("Failed to update log level to {level}: {err}");
    }
    level
}

struct LoggerRuntime {
    handle: flexi_logger::LoggerHandle,
    verbose: Mutex<u8>,
}

fn normalize_verbose(verbose: u8) -> u8 {
    verbose.min(2)
}

fn next_verbose(verbose: u8) -> u8 {
    match normalize_verbose(verbose) {
        0 => 1,
        1 => 2,
        _ => 0,
    }
}

fn log_level(verbose: u8) -> &'static str {
    match normalize_verbose(verbose) {
        0 => "info",
        1 => "debug",
        _ => "trace",
    }
}

#[derive(Debug, Clone, Copy)]
struct LogFlag(u8);

impl LogFlag {
    fn new(logs: &[Log]) -> Self {
        Self(logs.iter().fold(0, |acc, f| acc | f.flag()))
    }

    fn is_set(&self, log: Log) -> bool {
        log.flag() & self.0 != 0
    }

    fn matches(&self, target: &str) -> bool {
        self.do_server_log(target) || self.do_wasm_log(target)
    }

    fn do_server_log(&self, target: &str) -> bool {
        self.is_set(Log::Server) && (target.starts_with("hyper") || target.starts_with("salvo"))
    }

    fn do_wasm_log(&self, target: &str) -> bool {
        self.is_set(Log::Wasm) && (target.starts_with("wasm") || target.starts_with("walrus"))
    }
}

impl Log {
    fn flag(&self) -> u8 {
        match self {
            Self::Wasm => 0b0000_0001,
            Self::Server => 0b0000_0010,
        }
    }
}

// https://docs.rs/flexi_logger/0.24.1/flexi_logger/type.FormatFunction.html
fn format(write: &mut dyn Write, _now: &mut DeferredNow, record: &Record<'_>) -> Result<(), std::io::Error> {
    let args = record.args().to_string();

    let lvl_color = record.level().color();

    if let Some(dep) = dependency(record) {
        let dep = format!("[{}]", dep);
        let dep = dep.pad_left_to(12);
        write!(write, "{} {}", lvl_color.paint(dep), record.args())
    } else {
        let (word, rest) = split(&args);
        let word = word.pad_left_to(12);
        write!(write, "{} {}", lvl_color.paint(word), rest)
    }
}

fn split(args: &str) -> (&str, &str) {
    match args.find(' ') {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => ("", args),
    }
}
fn dependency<'a>(record: &'a Record<'_>) -> Option<&'a str> {
    let target = record.target();

    if !target.starts_with("glory_cli")
        && let Some((ent, _)) = target.split_once("::")
    {
        return Some(ent);
    }
    None
}

pub struct Filter;
impl LogLineFilter for Filter {
    fn write(&self, now: &mut DeferredNow, record: &Record, log_line_writer: &dyn LogLineWriter) -> std::io::Result<()> {
        let target = record.target();
        if record.level() == Level::Error
            || target.starts_with("glory_cli")
            // LOG_SELECT will have been initialized by now, get_or_init() not required
            || LOG_SELECT.get().is_some_and(|flag| flag.matches(target))
        {
            log_line_writer.write(now, record)?;
        }
        Ok(())
    }
}

trait LevelExt {
    fn color(&self) -> ansi_term::Color;
}

impl LevelExt for Level {
    fn color(&self) -> ansi_term::Color {
        match self {
            Level::Error => *ERR_RED,
            Level::Warn => *WARN_YELLOW,
            Level::Info => *INFO_GREEN,
            Level::Debug => *DBG_BLUE,
            Level::Trace => *TRACE_VIOLET,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbose_levels_cycle_through_info_debug_trace() {
        assert_eq!(log_level(0), "info");
        assert_eq!(log_level(1), "debug");
        assert_eq!(log_level(2), "trace");
        assert_eq!(log_level(9), "trace");

        assert_eq!(next_verbose(0), 1);
        assert_eq!(next_verbose(1), 2);
        assert_eq!(next_verbose(2), 0);
    }
}
