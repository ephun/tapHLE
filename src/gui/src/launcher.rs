/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Running an app, in its own process.
//!
//! The emulator is not something the frontend can call into. `Environment`
//! owns its process: it maps guest memory, drives an SDL event loop, and ends
//! a run by calling `std::process::exit`. One of those per process is the
//! design, and a library window that shares a process with a game would die
//! with it.
//!
//! So the frontend launches `tapHLE` the same way a person would from a
//! terminal, with the same arguments, and reads its output back through
//! pipes. That has four consequences worth stating, because they are the
//! reasons for the choice rather than side effects of it:
//!
//! * the emulator opens its own window, and the library window stays alive;
//! * a crash in a game cannot take the frontend with it, so the log and the
//!   diagnostics survive to be read afterwards;
//! * every option the frontend sets is an option the command line already
//!   accepts, so the two interfaces cannot drift apart;
//! * relaunching costs nothing, which is the whole developer loop.
//!
//! On Windows the child is created with no console (see [crate::process]), so
//! nothing flashes on screen.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Instant;

use crate::logstore::{self, LogLevel, LogOrigin, SharedLog};
use crate::process;

/// How a run ended.
#[derive(Clone, Debug, PartialEq)]
pub enum RunOutcome {
    /// The app closed itself, or the person closed its window.
    Finished,
    /// The emulator stopped early. The code is the process exit status.
    Failed { code: Option<i32> },
    /// The frontend asked it to stop.
    Stopped,
    /// The process could not be waited for at all.
    Lost(String),
}

impl RunOutcome {
    pub fn is_failure(&self) -> bool {
        matches!(self, RunOutcome::Failed { .. } | RunOutcome::Lost(_))
    }
}

/// One app running, or one that has just stopped.
pub struct Run {
    pub id: u64,
    pub entry_id: String,
    pub app_name: Arc<str>,
    pub started: Instant,
    pub arguments: Vec<String>,
    /// Set when the frontend asked for this run to end, so its non-zero exit
    /// is reported as a stop rather than as a crash.
    stopping: bool,
    child: Option<Child>,
}

impl Run {
    pub fn elapsed_seconds(&self) -> u64 {
        self.started.elapsed().as_secs()
    }
}

/// What one launch needs to know.
///
/// A struct rather than seven parameters: which emulator, where it runs,
/// which app and what to give it are separate concerns, and a call site
/// listing them positionally would be easy to get wrong.
pub struct LaunchRequest<'a> {
    pub emulator: &'a Path,
    /// The tapHLE installation directory, which is where the emulator looks
    /// for its dylibs, fonts and options files.
    pub working_directory: &'a Path,
    pub entry_id: &'a str,
    pub app_name: &'a str,
    pub app_path: &'a Path,
    /// The emulator options the settings asked for.
    pub arguments: &'a [String],
    pub environment: &'a [(String, String)],
}

/// Everything currently running, and the plumbing that watches it.
pub struct Launcher {
    runs: Vec<Run>,
    next_id: u64,
    log: SharedLog,
}

impl Launcher {
    pub fn new(log: SharedLog) -> Self {
        Launcher {
            runs: Vec::new(),
            next_id: 1,
            log,
        }
    }

    pub fn running(&self) -> &[Run] {
        &self.runs
    }

    pub fn is_running(&self, entry_id: &str) -> bool {
        self.runs.iter().any(|run| run.entry_id == entry_id)
    }

    pub fn any_running(&self) -> bool {
        !self.runs.is_empty()
    }

    /// Start an app.
    pub fn launch(&mut self, request: LaunchRequest<'_>) -> Result<u64, String> {
        let LaunchRequest {
            emulator,
            working_directory,
            entry_id,
            app_name,
            app_path,
            arguments,
            environment,
        } = request;
        if !emulator.exists() {
            return Err(format!(
                "The tapHLE emulator was not found at {}. Set its location in \
                 Settings ▸ Paths.",
                emulator.display()
            ));
        }

        let mut command = Command::new(emulator);
        command
            .arg(app_path)
            .args(arguments)
            .current_dir(working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in environment {
            command.env(name, value);
        }
        let mut child = process::without_console(&mut command)
            .spawn()
            .map_err(|e| format!("Could not start {}: {e}", emulator.display()))?;

        let id = self.next_id;
        self.next_id += 1;
        let app_name: Arc<str> = Arc::from(app_name);

        let origin = LogOrigin::Run {
            id,
            app: app_name.clone(),
        };
        // Both streams are read on their own threads. A pipe that is not
        // drained fills up and blocks the writer, which would freeze the
        // emulator as soon as it had printed a few kilobytes.
        if let Some(stdout) = child.stdout.take() {
            spawn_reader(stdout, self.log.clone(), origin.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_reader(stderr, self.log.clone(), origin.clone());
        }

        logstore::note(
            &self.log,
            LogLevel::Info,
            format!(
                "Launching {app_name}: {} {}",
                app_path.display(),
                arguments.join(" ")
            ),
        );

        self.runs.push(Run {
            id,
            entry_id: entry_id.to_string(),
            app_name,
            started: Instant::now(),
            arguments: arguments.to_vec(),
            stopping: false,
            child: Some(child),
        });
        Ok(id)
    }

    /// Ask a run to stop.
    ///
    /// There is no polite way to ask: the emulator has no control channel,
    /// and a guest app in a tight loop would ignore one anyway. The window's
    /// close button is the graceful path and this is the button for when
    /// that has stopped working, so it terminates.
    pub fn stop(&mut self, run_id: u64) {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == run_id) else {
            return;
        };
        run.stopping = true;
        if let Some(child) = run.child.as_mut() {
            let name = run.app_name.clone();
            match child.kill() {
                Ok(()) => logstore::note(&self.log, LogLevel::Info, format!("Stopped {name}.")),
                Err(e) => logstore::note(
                    &self.log,
                    LogLevel::Warning,
                    format!("Could not stop {name}: {e}"),
                ),
            }
        }
    }

    pub fn stop_all(&mut self) {
        let ids: Vec<u64> = self.runs.iter().map(|run| run.id).collect();
        for id in ids {
            self.stop(id);
        }
    }

    /// Collect any run that has ended.
    ///
    /// Called once per frame. Waiting is non-blocking, so a running app never
    /// holds up the interface.
    pub fn poll(&mut self) -> Vec<(Run, RunOutcome)> {
        let mut ended = Vec::new();
        let mut index = 0;
        while index < self.runs.len() {
            let status = match self.runs[index].child.as_mut() {
                Some(child) => child.try_wait(),
                None => Ok(None),
            };
            match status {
                Ok(Some(status)) => {
                    let outcome = if status.success() {
                        RunOutcome::Finished
                    } else if self.runs[index].stopping {
                        // Terminating a process gives it a non-zero status.
                        // Reporting that as a crash would put a diagnostic
                        // dialog in front of somebody who pressed Stop.
                        RunOutcome::Stopped
                    } else {
                        RunOutcome::Failed {
                            code: status.code(),
                        }
                    };
                    ended.push((self.runs.remove(index), outcome));
                }
                Ok(None) => index += 1,
                Err(e) => {
                    ended.push((self.runs.remove(index), RunOutcome::Lost(e.to_string())));
                }
            }
        }
        ended
    }
}

fn spawn_reader<R: std::io::Read + Send + 'static>(stream: R, log: SharedLog, origin: LogOrigin) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        // Bytes rather than lines: a guest app can print anything, and one
        // invalid UTF-8 sequence must not end the capture for the whole run.
        for line in reader.split(b'\n') {
            let Ok(bytes) = line else { break };
            let text = String::from_utf8_lossy(&bytes);
            logstore::append(&log, origin.clone(), text.trim_end_matches('\r'));
        }
    });
}

/// Where the emulator executable is.
///
/// The frontend and the emulator are built and installed side by side, so the
/// answer is almost always "next to this program". The current directory is
/// tried as well, which is what makes an unpacked release directory work when
/// the frontend was started from somewhere else.
pub fn find_emulator(configured: Option<&Path>, data_dir: &Path) -> Option<PathBuf> {
    if let Some(path) = configured {
        return path.exists().then(|| path.to_path_buf());
    }
    let name = if cfg!(windows) {
        "tapHLE.exe"
    } else {
        "tapHLE"
    };
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(name));
        }
    }
    candidates.push(data_dir.join(name));
    candidates.into_iter().find(|path| path.is_file())
}

/// The message shown when a run ends badly.
///
/// The exit codes are the emulator's own: it calls `exit(-1)` for an error it
/// has already explained in the log, and Rust's runtime uses 101 for a panic.
/// Naming them turns "it vanished" into something a person can act on.
pub fn explain_outcome(outcome: &RunOutcome, app: &str) -> String {
    match outcome {
        RunOutcome::Finished => format!("{app} closed."),
        RunOutcome::Stopped => format!("{app} was stopped."),
        RunOutcome::Lost(reason) => {
            format!("tapHLE lost track of {app}: {reason}")
        }
        RunOutcome::Failed { code } => {
            let detail = match code {
                Some(101) => {
                    " It hit something tapHLE has not implemented, or an internal check \
                     failed. The last lines of the log say which."
                }
                Some(-1) | Some(255) => {
                    " The emulator reported an error; the log says what it was."
                }
                _ => " The log's last lines are the best clue.",
            };
            let code = match code {
                Some(code) => format!(" (exit code {code})"),
                None => " (it was terminated)".to_string(),
            };
            format!("{app} stopped unexpectedly{code}.{detail}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logstore::{self, LogLevel};

    /// A stand-in for the emulator: a program every machine has, which
    /// prints something and exits with a status we choose.
    ///
    /// The point of these tests is the plumbing — spawning, the pipes, the
    /// log capture and the exit status — not the emulator, so using a
    /// predictable program keeps them fast and keeps them passing on a
    /// machine that has not built tapHLE yet.
    /// The program, and the flag that has to come before the script.
    ///
    /// A real launch is `tapHLE <app path> <options>`, so the flag takes the
    /// app path's place and the script takes the options'.
    fn shell() -> (PathBuf, PathBuf) {
        if cfg!(windows) {
            // COMSPEC rather than a written-out path: cmd.exe resolves things
            // from its own command line and fails with "the system cannot
            // find the path specified" when it is given one with forward
            // slashes, which is easy to write and hard to diagnose.
            let comspec = std::env::var("COMSPEC")
                .unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_string());
            (PathBuf::from(comspec), PathBuf::from("/C"))
        } else {
            (PathBuf::from("/bin/sh"), PathBuf::from("-c"))
        }
    }

    /// Start a command through the shell, as if it were an app.
    ///
    /// `words` are passed separately on Windows and joined into one argument
    /// on Unix, because the two shells want opposite things: `sh -c` takes
    /// the whole script as a single argument, while `cmd /C` given a single
    /// quoted argument treats it as the name of a program to run.
    fn start(launcher: &mut Launcher, app_name: &str, words: &[&str]) -> u64 {
        let (program, flag) = shell();
        let arguments: Vec<String> = if cfg!(windows) {
            words.iter().map(|word| word.to_string()).collect()
        } else {
            vec![words.join(" ")]
        };
        launcher
            .launch(LaunchRequest {
                emulator: &program,
                working_directory: &std::env::temp_dir(),
                entry_id: "com.test@1",
                app_name,
                app_path: &flag,
                arguments: &arguments,
                environment: &[],
            })
            .expect("the shell should start")
    }

    /// Wait for the one running run to end.
    fn wait_for_end(launcher: &mut Launcher) -> RunOutcome {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            if let Some((_, outcome)) = launcher.poll().into_iter().next() {
                return outcome;
            }
            assert!(std::time::Instant::now() < deadline, "the run never ended");
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }

    fn run_to_completion(words: &[&str]) -> (RunOutcome, Vec<String>) {
        let log = logstore::new_shared();
        let mut launcher = Launcher::new(log.clone());
        start(&mut launcher, "Test App", words);
        let outcome = wait_for_end(&mut launcher);
        // The reader threads may still be draining the pipe when the process
        // ends, so give them a moment to finish.
        std::thread::sleep(std::time::Duration::from_millis(400));
        let lines = log
            .lock()
            .unwrap()
            .iter()
            .map(|line| line.full_text())
            .collect();
        (outcome, lines)
    }

    /// The whole reason the emulator runs in its own process is that its
    /// output can be read back. If this breaks, the log panel is empty and
    /// there is no way to tell why an app failed.
    #[test]
    fn a_run_s_output_reaches_the_log() {
        let (outcome, lines) =
            run_to_completion(&["echo", "tapHLE::test:", "hello", "from", "the", "child"]);
        assert_eq!(outcome, RunOutcome::Finished);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("hello from the child")),
            "the child's output should be in the log, got {lines:?}"
        );
    }

    /// Every line is tagged with the run it came from, which is what the
    /// panel's per-app filter and a crash excerpt depend on.
    #[test]
    fn captured_lines_are_attributed_to_the_run() {
        let log = logstore::new_shared();
        let mut launcher = Launcher::new(log.clone());
        let id = start(&mut launcher, "Named App", &["echo", "attributed"]);
        wait_for_end(&mut launcher);
        std::thread::sleep(std::time::Duration::from_millis(400));
        let store = log.lock().unwrap();
        // Only the child's own line, not the frontend's note about starting
        // it, which quotes the same command.
        let from_run = store
            .iter()
            .find(|line| line.origin.run_id().is_some() && line.message.contains("attributed"))
            .expect("the child's line should be present");
        assert_eq!(from_run.origin.run_id(), Some(id));
        assert_eq!(from_run.origin.label(), "Named App");
    }

    /// A non-zero exit is what a crashed app looks like from here, and it
    /// has to be reported as a failure so the crash notice appears.
    #[test]
    fn a_non_zero_exit_is_a_failure() {
        let (outcome, _) = run_to_completion(&["exit", "3"]);
        assert_eq!(outcome, RunOutcome::Failed { code: Some(3) });
        assert!(outcome.is_failure());
    }

    /// Stopping a run also produces a non-zero status, and reporting that as
    /// a crash would put a diagnostic dialog in front of somebody who had
    /// just pressed Stop.
    #[test]
    fn stopping_a_run_is_not_reported_as_a_crash() {
        let log = logstore::new_shared();
        let mut launcher = Launcher::new(log);
        let script: &[&str] = if cfg!(windows) {
            &["ping", "-n", "30", "127.0.0.1"]
        } else {
            &["sleep", "30"]
        };
        let id = start(&mut launcher, "Long App", script);
        assert!(launcher.is_running("com.test@1"));
        launcher.stop(id);
        let outcome = wait_for_end(&mut launcher);
        assert_eq!(outcome, RunOutcome::Stopped);
        assert!(!outcome.is_failure());
    }

    /// The frontend has to notice a run has ended so it can record the time
    /// played and stop showing the app as running.
    #[test]
    fn a_finished_run_is_no_longer_running() {
        let log = logstore::new_shared();
        let mut launcher = Launcher::new(log);
        start(&mut launcher, "Test App", &["echo", "done"]);
        assert!(launcher.any_running());
        wait_for_end(&mut launcher);
        assert!(!launcher.any_running());
        assert!(!launcher.is_running("com.test@1"));
    }

    /// An emulator that is not there has to be reported before anything is
    /// spawned, with a message that says what to do about it.
    #[test]
    fn a_missing_emulator_is_reported_clearly() {
        let log = logstore::new_shared();
        let mut launcher = Launcher::new(log);
        let error = launcher
            .launch(LaunchRequest {
                emulator: Path::new("Z:/nowhere/tapHLE.exe"),
                working_directory: &std::env::temp_dir(),
                entry_id: "com.test@1",
                app_name: "Test App",
                app_path: Path::new("Z:/nowhere/Game.ipa"),
                arguments: &[],
                environment: &[],
            })
            .unwrap_err();
        assert!(error.contains("was not found"), "got {error:?}");
        assert!(error.contains("Settings"), "it should say where to fix it");
    }

    /// The frontend logs what it is about to run, so a launch that fails
    /// leaves a record of the exact arguments it used.
    #[test]
    fn the_launch_itself_is_logged() {
        let log = logstore::new_shared();
        let mut launcher = Launcher::new(log.clone());
        start(&mut launcher, "Test App", &["echo", "x"]);
        let store = log.lock().unwrap();
        let line = store
            .iter()
            .find(|line| line.message.starts_with("Launching"))
            .expect("the launch should be logged");
        assert_eq!(line.level, LogLevel::Info);
        assert!(line.message.contains("Test App"));
        drop(store);
        launcher.stop_all();
    }

    #[test]
    fn a_clean_exit_is_not_a_failure() {
        assert!(!RunOutcome::Finished.is_failure());
        assert!(!RunOutcome::Stopped.is_failure());
        assert!(RunOutcome::Failed { code: Some(101) }.is_failure());
    }

    /// A crash message has to say something a person can act on. The panic
    /// exit code is the common case in compatibility work and deserves its
    /// own wording.
    #[test]
    fn a_panic_is_explained_as_a_missing_feature() {
        let text = explain_outcome(&RunOutcome::Failed { code: Some(101) }, "Ricky");
        assert!(text.contains("Ricky"));
        assert!(text.contains("not implemented"));
        assert!(text.contains("101"));
    }

    #[test]
    fn a_termination_without_a_code_is_still_explained() {
        let text = explain_outcome(&RunOutcome::Failed { code: None }, "Ricky");
        assert!(text.contains("terminated"));
    }

    /// A configured path that does not exist must not be returned, or every
    /// launch fails with a confusing error from the operating system.
    #[test]
    fn a_configured_emulator_must_exist() {
        let missing = Path::new("Z:/definitely/not/here/tapHLE.exe");
        assert_eq!(find_emulator(Some(missing), Path::new(".")), None);
    }
}
