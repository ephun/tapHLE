/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The buffer every line of output ends up in, and how a line is classified.
//!
//! There is one store for the whole frontend. Emulator processes write to it
//! from reader threads, the frontend writes its own messages to it, and the
//! log panel reads it. That is what lets output keep arriving while the panel
//! is collapsed, and what keeps a crashed app's output available after its
//! window has gone.
//!
//! tapHLE has no structured logging: `log!` prints `module_path!()`, a colon
//! and a message, and everything else goes through `echo!` unadorned. Rather
//! than invent a second logging system for the emulator to write to, the
//! store recovers the structure from the text — the module prefix is exact,
//! and the severity is inferred from the wording. [classify] is where that
//! guesswork lives, and it is deliberately the only place.

use crate::timefmt;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// How serious a line is. Ordered, so a filter can say "warnings and worse".
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn label(self) -> &'static str {
        match self {
            LogLevel::Debug => "Debug",
            LogLevel::Info => "Info",
            LogLevel::Warning => "Warning",
            LogLevel::Error => "Error",
        }
    }

    /// A short marker for the exported text, so severity survives a copy and
    /// paste into an issue or a compatibility report.
    pub fn marker(self) -> &'static str {
        match self {
            LogLevel::Debug => "dbg",
            LogLevel::Info => "   ",
            LogLevel::Warning => "WRN",
            LogLevel::Error => "ERR",
        }
    }
}

/// Which process a line came from.
#[derive(Clone, PartialEq, Eq)]
pub enum LogOrigin {
    /// The frontend itself.
    Frontend,
    /// One run of one app. The identifier distinguishes two runs of the same
    /// app, which is what "filter to the running application" needs.
    Run { id: u64, app: Arc<str> },
}

impl LogOrigin {
    pub fn run_id(&self) -> Option<u64> {
        match self {
            LogOrigin::Frontend => None,
            LogOrigin::Run { id, .. } => Some(*id),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            LogOrigin::Frontend => "tapHLE",
            LogOrigin::Run { app, .. } => app,
        }
    }
}

pub struct LogLine {
    /// Position in the whole stream, never reused. Used instead of an index
    /// because the buffer drops its oldest lines.
    pub seq: u64,
    pub millis: u64,
    pub origin: LogOrigin,
    pub level: LogLevel,
    /// The `tapHLE::...` module a `log!` line named, when there was one.
    pub module: Option<Box<str>>,
    /// The line with any module prefix removed.
    pub message: Box<str>,
}

impl LogLine {
    /// The line as it was printed, which is what an export and a clipboard
    /// copy should contain.
    pub fn full_text(&self) -> String {
        match &self.module {
            Some(module) => format!("{module}: {}", self.message),
            None => self.message.to_string(),
        }
    }
}

/// Wordings that mean something failed. Checked before the warning list, so a
/// line matching both is an error.
///
/// These are matched case-insensitively against the message. They cover
/// Rust's own panic and assertion output, which is how an unimplemented call
/// or a failed invariant reaches the log, plus the emulator's own fatal
/// wordings.
const ERROR_PATTERNS: &[&str] = &[
    "panicked at",
    "assertion failed",
    "assertion `",
    "not implemented",
    "unimplemented",
    "fatal",
    "aborted",
    "segmentation fault",
    "stack overflow",
    "could not open app bundle",
    "application bundle error",
];

/// Wordings that mean something is missing or suspect but the app is still
/// running. `log!("Warning: ...")` is the emulator's convention.
const WARNING_PATTERNS: &[&str] = &[
    "warning:",
    "todo",
    "unsupported",
    "ignoring",
    "unrecognised",
];

/// Split a printed line into its module and severity.
///
/// The module prefix is recognised only when it looks like one of tapHLE's
/// own module paths, so a message that merely contains a colon keeps its text
/// intact.
pub fn classify(line: &str) -> (Option<&str>, &str, LogLevel) {
    let (module, message) = match line.split_once(": ") {
        Some((prefix, rest)) if prefix.starts_with("tapHLE::") && !prefix.contains(' ') => {
            (Some(prefix), rest)
        }
        _ => (None, line),
    };
    (module, message, level_of(message))
}

fn level_of(message: &str) -> LogLevel {
    let lowered = message.to_ascii_lowercase();
    if ERROR_PATTERNS.iter().any(|p| lowered.contains(p)) {
        LogLevel::Error
    } else if WARNING_PATTERNS.iter().any(|p| lowered.contains(p)) {
        LogLevel::Warning
    } else {
        LogLevel::Info
    }
}

/// The default number of lines kept. A verbose app produces a few thousand
/// lines a minute, so this is roughly an hour of output; the memory cost is a
/// few tens of megabytes at worst.
pub const DEFAULT_CAPACITY: usize = 200_000;

pub struct LogStore {
    lines: VecDeque<LogLine>,
    capacity: usize,
    next_seq: u64,
    dropped: u64,
    errors: u64,
    warnings: u64,
}

impl Default for LogStore {
    fn default() -> Self {
        LogStore {
            lines: VecDeque::new(),
            capacity: DEFAULT_CAPACITY,
            next_seq: 0,
            dropped: 0,
            errors: 0,
            warnings: 0,
        }
    }
}

impl LogStore {
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.max(1000);
        self.trim();
    }

    fn trim(&mut self) {
        while self.lines.len() > self.capacity {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }

    /// Add a line of already-printed output.
    pub fn push_raw(&mut self, origin: LogOrigin, text: &str) {
        let (module, message, level) = classify(text);
        let line = LogLine {
            seq: self.next_seq,
            millis: timefmt::now_millis(),
            origin,
            level,
            module: module.map(Box::from),
            message: Box::from(message),
        };
        self.push_line(line);
    }

    /// Add a line the frontend generated itself, whose severity is known.
    pub fn push_frontend(&mut self, level: LogLevel, text: impl Into<String>) {
        let line = LogLine {
            seq: self.next_seq,
            millis: timefmt::now_millis(),
            origin: LogOrigin::Frontend,
            level,
            module: None,
            message: Box::from(text.into().as_str()),
        };
        self.push_line(line);
    }

    fn push_line(&mut self, line: LogLine) {
        match line.level {
            LogLevel::Error => self.errors += 1,
            LogLevel::Warning => self.warnings += 1,
            _ => (),
        }
        self.next_seq += 1;
        self.lines.push_back(line);
        self.trim();
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.errors = 0;
        self.warnings = 0;
        self.dropped = 0;
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The sequence number of the oldest line still held.
    pub fn first_seq(&self) -> u64 {
        self.lines.front().map_or(self.next_seq, |line| line.seq)
    }

    /// One past the newest sequence number, i.e. the next to be issued.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    pub fn get(&self, seq: u64) -> Option<&LogLine> {
        let first = self.first_seq();
        if seq < first {
            return None;
        }
        self.lines.get((seq - first) as usize)
    }

    pub fn errors(&self) -> u64 {
        self.errors
    }

    pub fn warnings(&self) -> u64 {
        self.warnings
    }

    /// How many lines have been discarded to stay within capacity, so the
    /// panel can say so rather than silently losing the start of a run.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn iter(&self) -> impl Iterator<Item = &LogLine> {
        self.lines.iter()
    }
}

/// The store as everything else sees it.
pub type SharedLog = Arc<Mutex<LogStore>>;

pub fn new_shared() -> SharedLog {
    Arc::new(Mutex::new(LogStore::default()))
}

/// Append to the shared store, ignoring a poisoned lock.
///
/// A panic in one reader thread must not silence every later message, and
/// there is nothing useful to do about it here.
pub fn append(log: &SharedLog, origin: LogOrigin, text: &str) {
    if let Ok(mut store) = log.lock() {
        store.push_raw(origin, text);
    }
}

pub fn note(log: &SharedLog, level: LogLevel, text: impl Into<String>) {
    if let Ok(mut store) = log.lock() {
        store.push_frontend(level, text);
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, LogLevel, LogOrigin, LogStore};

    /// `log!` prints the module path, so the panel can offer a subsystem
    /// filter without the emulator having to learn about the frontend.
    #[test]
    fn a_module_prefix_is_recognized() {
        let (module, message, _) = classify("tapHLE::frameworks::uikit: hello there");
        assert_eq!(module, Some("tapHLE::frameworks::uikit"));
        assert_eq!(message, "hello there");
    }

    /// A message that merely contains a colon is not a module prefix, and
    /// must keep its whole text.
    #[test]
    fn an_ordinary_colon_is_not_a_module() {
        let (module, message, _) = classify("- Display name: Some Game");
        assert_eq!(module, None);
        assert_eq!(message, "- Display name: Some Game");
    }

    #[test]
    fn warnings_and_errors_are_told_apart() {
        assert_eq!(
            classify("tapHLE::mem: Warning: odd size").2,
            LogLevel::Warning
        );
        assert_eq!(
            classify("thread 'main' panicked at src/x.rs:1:1").2,
            LogLevel::Error
        );
        assert_eq!(
            classify("tapHLE::objc: not implemented: -[Foo bar]").2,
            LogLevel::Error
        );
        assert_eq!(classify("App bundle info:").2, LogLevel::Info);
    }

    /// An unimplemented call is the single most important thing to find in a
    /// compatibility session, and it arrives as a panic, so it must not be
    /// downgraded by also matching a warning wording.
    #[test]
    fn an_unimplemented_call_outranks_a_warning_wording() {
        let (_, _, level) = classify("tapHLE::x: unimplemented: ignoring this property");
        assert_eq!(level, LogLevel::Error);
    }

    /// The buffer is bounded, and what it drops has to be reported rather
    /// than silently lost, or a long run's diagnostics become misleading.
    #[test]
    fn the_oldest_lines_are_dropped_and_counted() {
        let mut store = LogStore::default();
        store.set_capacity(1000);
        for i in 0..1100 {
            store.push_raw(LogOrigin::Frontend, &format!("line {i}"));
        }
        assert_eq!(store.len(), 1000);
        assert_eq!(store.dropped(), 100);
        assert_eq!(store.first_seq(), 100);
        assert_eq!(store.get(100).unwrap().message.as_ref(), "line 100");
        assert!(store.get(99).is_none());
    }

    /// Sequence numbers must index the buffer correctly after a drop; the
    /// log panel addresses lines by sequence number and would otherwise show
    /// the wrong ones as soon as the buffer filled.
    #[test]
    fn sequence_numbers_survive_dropping() {
        let mut store = LogStore::default();
        store.set_capacity(1000);
        for i in 0..2000 {
            store.push_raw(LogOrigin::Frontend, &format!("line {i}"));
        }
        for seq in store.first_seq()..store.next_seq() {
            let line = store.get(seq).expect("line should be present");
            assert_eq!(line.message.as_ref(), format!("line {seq}"));
        }
    }

    #[test]
    fn error_and_warning_counts_are_kept() {
        let mut store = LogStore::default();
        store.push_raw(LogOrigin::Frontend, "Warning: something");
        store.push_raw(LogOrigin::Frontend, "thread 'main' panicked at lib.rs");
        store.push_raw(LogOrigin::Frontend, "ordinary line");
        assert_eq!(store.warnings(), 1);
        assert_eq!(store.errors(), 1);
        store.clear();
        assert_eq!(store.warnings(), 0);
        assert_eq!(store.errors(), 0);
    }
}
