/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Getting a picture of an app's screen, so controls can be placed on it.
//!
//! Placing a touch target on a black rectangle is guesswork. The whole point
//! of a visual editor is to drag the marker onto the app's own button, and
//! that needs the app's own screen underneath.
//!
//! The emulator can already do this. It watches for a marker file and writes
//! the next presented frame out as a PPM, which is how the debugging harness
//! takes screenshots. Nothing about it needed to be a harness-only feature:
//! it is armed by two environment variables, and the frontend launches the
//! emulator with an environment already.
//!
//! The run is deliberately its own short-lived process rather than the one
//! somebody pressed Play on. It is not a play session — it should not count
//! towards how long an app has been played — and the environment has to be
//! set before launch, so an already-running app could not be asked anyway.
//!
//! **The person decides when the picture is taken.** An earlier version
//! waited a fixed six seconds and then shot whatever was on screen, which
//! meant the picture was usually a publisher logo, and on a slow app the run
//! was killed before it had drawn its menu at all. The screen worth mapping
//! controls onto is a screen only the person watching can recognise, so the
//! app is left running, they play it to wherever they want, and they press a
//! button. Nothing here is on a timer until they do.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// How long to wait for a frame after asking for one. Generous, because a
/// frame can be behind a shader compile. This starts when the picture is
/// asked for, never before.
const DEADLINE: Duration = Duration::from_secs(20);

pub struct Frame {
    pub width: usize,
    pub height: usize,
    /// Row-major RGBA, top row first.
    pub rgba: Vec<u8>,
}

pub enum Progress {
    /// The app is running and nobody has asked for a picture yet.
    Running,
    /// A picture has been asked for and has not arrived yet.
    Taking,
    Ready(Frame),
    Failed(String),
}

#[derive(PartialEq)]
enum Stage {
    Running,
    Requested,
    /// A result has been handed over; there is nothing left to poll.
    Finished,
}

/// An app running so a picture can be taken of it.
///
/// Nothing here blocks. The process is started, and then polled from wherever
/// the interface is already being drawn, because the whole point is that the
/// app keeps running — and stays interactive — until somebody says when.
pub struct Capture {
    child: Child,
    /// Everything this capture made, removed when it is dropped.
    directory: PathBuf,
    request: PathBuf,
    output: PathBuf,
    log: PathBuf,
    stage: Stage,
    /// When the picture was asked for, for the deadline.
    requested_at: Option<Instant>,
}

impl Capture {
    /// Start the app. It keeps running until a picture is taken or this is
    /// dropped.
    pub fn start(
        emulator: PathBuf,
        app_path: PathBuf,
        working_directory: PathBuf,
        arguments: Vec<String>,
    ) -> Result<Capture, String> {
        let directory = std::env::temp_dir().join(format!(
            "taphle-editor-capture-{}",
            std::process::id() as u64 * 31 + now_nanos()
        ));
        std::fs::create_dir_all(&directory)
            .map_err(|e| format!("Could not make a place for the capture: {e}"))?;

        let request = directory.join("frame.request");
        let output = directory.join("frame.ppm");

        // Kept, so a failure can say what the emulator said. A capture that
        // fails with only an exit code leaves somebody with nothing to act
        // on, and the reason is almost always in the last few lines.
        let log_path = directory.join("log.txt");
        let log = std::fs::File::create(&log_path)
            .map_err(|e| format!("Could not open a log for the capture: {e}"))?;
        let log_err = log
            .try_clone()
            .map_err(|e| format!("Could not open a log for the capture: {e}"))?;

        let mut command = Command::new(emulator);
        command
            .arg(app_path)
            .args(arguments)
            .current_dir(working_directory)
            .env("TAPHLE_FRAME_CAPTURE_REQUEST", &request)
            .env("TAPHLE_FRAME_CAPTURE_OUTPUT", &output)
            .stdout(std::process::Stdio::from(log))
            .stderr(std::process::Stdio::from(log_err));
        crate::platform::process::without_console(&mut command);

        let child = command.spawn().map_err(|e| {
            // Only ever this directory, by the path just built. Never a
            // pattern match against the temporary directory, which is shared.
            let _ = std::fs::remove_dir_all(&directory);
            format!("Could not start the emulator: {e}")
        })?;

        Ok(Capture {
            child,
            directory,
            request,
            output,
            log: log_path,
            stage: Stage::Running,
            requested_at: None,
        })
    }

    /// Whether asking for a picture would do anything.
    pub fn can_take(&self) -> bool {
        self.stage == Stage::Running
    }

    /// Ask for the next frame the app draws.
    pub fn take_now(&mut self) -> Result<(), String> {
        if !self.can_take() {
            return Ok(());
        }
        // Created, not written to: the emulator treats the file appearing as
        // the request, and refuses to overwrite an existing output.
        std::fs::File::create(&self.request)
            .map_err(|e| format!("Could not ask for a frame: {e}"))?;
        self.stage = Stage::Requested;
        self.requested_at = Some(Instant::now());
        Ok(())
    }

    pub fn poll(&mut self) -> Progress {
        match self.stage {
            Stage::Finished => Progress::Failed("The capture already finished.".to_string()),
            Stage::Running => match self.child.try_wait() {
                Ok(Some(status)) => self.fail(format!(
                    "The app closed before a picture was taken ({status})."
                )),
                _ => Progress::Running,
            },
            Stage::Requested => self.poll_requested(),
        }
    }

    fn poll_requested(&mut self) -> Progress {
        if self.output.is_file() {
            // The file becomes visible before it is fully written, so a parse
            // failure here means "not finished yet" rather than "broken".
            if let Ok(bytes) = std::fs::read(&self.output) {
                if let Ok(frame) = decode_ppm(&bytes) {
                    self.stage = Stage::Finished;
                    return Progress::Ready(frame);
                }
            }
        }
        if let Ok(Some(status)) = self.child.try_wait() {
            return self.fail(format!(
                "The app stopped before the frame arrived ({status})."
            ));
        }
        let waited = self.requested_at.map(|at| at.elapsed()).unwrap_or_default();
        if waited > DEADLINE {
            return self.fail("The app did not produce a frame in time.".to_string());
        }
        Progress::Taking
    }

    /// Finish with a message, saying what the emulator said if it said
    /// anything useful.
    fn fail(&mut self, reason: String) -> Progress {
        self.stage = Stage::Finished;
        match last_meaningful_line(&self.log) {
            Some(said) => Progress::Failed(format!("{reason}\n{said}")),
            None => Progress::Failed(reason),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // The app is only running because this editor asked it to, so it goes
        // when the editor is done with it, however that happened — a picture
        // taken, a failure, or the editor closed with the app still up.
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Only ever this directory, by the path `start` built. Never a
        // pattern match against the temporary directory, which is shared.
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// The last line of the emulator's output that is likely to explain a
/// failure, so the message can say more than an exit code.
fn last_meaningful_line(log: &Path) -> Option<String> {
    let text = std::fs::read_to_string(log).ok()?;
    let interesting = |line: &&str| {
        let l = line.trim();
        !l.is_empty() && !l.starts_with("tapHLE::")
    };
    // A panic explains itself; otherwise the last non-trace line will do.
    let panicked = text.lines().find(|l| l.contains("panicked at"));
    let last = text.lines().rfind(interesting);
    panicked.or(last).map(|l| l.trim().to_string())
}

fn now_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
}

/// Decode the binary PPM the emulator writes, and turn it the right way up.
///
/// The capture has OpenGL's pixel origin, which is the bottom-left, so the
/// rows arrive in the opposite order from the one every image API expects.
/// Handing it over unflipped would put the app's top row at the bottom of the
/// canvas, and every control would be placed mirrored vertically.
fn decode_ppm(bytes: &[u8]) -> Result<Frame, String> {
    let mut fields = Vec::new();
    let mut at = 0;
    // A PPM header is `P6` and three numbers, separated by any whitespace,
    // with `#` comments allowed between them.
    while fields.len() < 4 && at < bytes.len() {
        match bytes[at] {
            b'#' => {
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
            }
            c if c.is_ascii_whitespace() => at += 1,
            _ => {
                let start = at;
                while at < bytes.len() && !bytes[at].is_ascii_whitespace() {
                    at += 1;
                }
                fields.push(
                    std::str::from_utf8(&bytes[start..at])
                        .map_err(|_| "The frame's header is not text.".to_string())?
                        .to_string(),
                );
            }
        }
    }
    if fields.len() < 4 {
        return Err("The frame is incomplete.".to_string());
    }
    if fields[0] != "P6" {
        return Err(format!("Expected a P6 frame, found {}.", fields[0]));
    }
    let parse = |s: &String| -> Result<usize, String> {
        s.parse().map_err(|_| format!("{s} is not a number."))
    };
    let width = parse(&fields[1])?;
    let height = parse(&fields[2])?;
    let max = parse(&fields[3])?;
    if max != 255 {
        return Err(format!("Only 8-bit frames are understood, not {max}."));
    }
    // Exactly one whitespace character separates the header from the pixels.
    at += 1;

    let expected = width * height * 3;
    if bytes.len() < at + expected {
        return Err("The frame is still being written.".to_string());
    }
    let pixels = &bytes[at..at + expected];

    let mut rgba = vec![0u8; width * height * 4];
    for y in 0..height {
        let source_row = height - 1 - y;
        for x in 0..width {
            let from = (source_row * width + x) * 3;
            let to = (y * width + x) * 4;
            rgba[to] = pixels[from];
            rgba[to + 1] = pixels[from + 1];
            rgba[to + 2] = pixels[from + 2];
            rgba[to + 3] = 255;
        }
    }
    Ok(Frame {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ppm(width: usize, height: usize, rows: &[[u8; 3]]) -> Vec<u8> {
        let mut out = format!("P6\n{width} {height}\n255\n").into_bytes();
        for row in rows {
            for _ in 0..width {
                out.extend_from_slice(row);
            }
        }
        out
    }

    /// The emulator writes rows bottom-up, because that is OpenGL's origin.
    /// Getting this backwards is not a crash — it is a picture that looks
    /// almost right, and every control placed on it ends up mirrored.
    #[test]
    fn a_frame_is_turned_the_right_way_up() {
        // Bottom row red, top row blue, as OpenGL would hand it over.
        let bytes = ppm(2, 2, &[[255, 0, 0], [0, 0, 255]]);
        let frame = decode_ppm(&bytes).expect("should decode");
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(
            &frame.rgba[0..4],
            &[0, 0, 255, 255],
            "top row should be blue"
        );
        let second_row = 2 * 4;
        assert_eq!(
            &frame.rgba[second_row..second_row + 4],
            &[255, 0, 0, 255],
            "bottom row should be red"
        );
    }

    /// The output file appears before it has been fully written, so a short
    /// read has to read as "not yet" rather than as a broken frame.
    #[test]
    fn a_half_written_frame_is_an_error_not_a_panic() {
        let bytes = ppm(4, 4, &[[1, 2, 3]]);
        for cut in [0, 3, 10, bytes.len() - 1] {
            assert!(decode_ppm(&bytes[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn a_header_with_comments_and_odd_spacing_still_reads() {
        let mut bytes = b"P6\n# written by tapHLE\n2  2\n255\n".to_vec();
        bytes.extend(std::iter::repeat_n(0u8, 2 * 2 * 3));
        let frame = decode_ppm(&bytes).expect("should decode");
        assert_eq!((frame.width, frame.height), (2, 2));
    }

    #[test]
    fn something_that_is_not_a_frame_is_refused() {
        assert!(decode_ppm(b"not a ppm at all").is_err());
        assert!(decode_ppm(b"P3\n2 2\n255\n").is_err(), "only binary P6");
        assert!(decode_ppm(b"P6\n2 2\n65535\n").is_err(), "only 8-bit");
    }
}
