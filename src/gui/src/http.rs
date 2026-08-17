/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The one place the frontend makes a network request.
//!
//! Two features need to read a URL: the compatibility ratings and the update
//! check. Both are optional, both are read-only, and both must be able to
//! fail without affecting anything else. That is a poor trade for linking a
//! TLS stack into the frontend — it would be the largest dependency in the
//! program, and the emulator itself has no use for one — so requests are made
//! through `curl`, which ships with Windows 10 and later, with macOS, and
//! with essentially every Linux distribution.
//!
//! The cost of that choice is that a machine without `curl` gets no ratings
//! and no update check. Both failures are reported and neither is fatal.
//! [Transport] is a trait so a linked-in client can replace this later
//! without either caller changing.

use crate::process;
use std::process::{Command, Stdio};

pub struct Response {
    pub status: u16,
    pub body: String,
}

/// How a URL is fetched. The trait exists so that the callers depend on the
/// idea of fetching rather than on `curl`.
pub trait Transport: Send + Sync {
    /// A short description of the transport, for the About window and for
    /// error messages.
    fn describe(&self) -> String;

    /// Whether requests can be made at all.
    fn is_available(&self) -> bool;

    fn get(&self, url: &str, timeout_seconds: u32) -> Result<Response, String>;
}

/// The user agent sent with every request, so the maintainer can tell
/// frontend traffic apart in a server log.
fn user_agent() -> String {
    format!("tapHLE-gui/{}", tapHLE_version::VERSION.trim())
}

/// A transport that runs the system's `curl`.
pub struct CurlTransport;

impl CurlTransport {
    /// Whether `curl` can be run, checked once.
    pub fn probe() -> bool {
        let mut command = Command::new("curl");
        command
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        process::without_console(&mut command)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

impl Transport for CurlTransport {
    fn describe(&self) -> String {
        "system curl".to_string()
    }

    fn is_available(&self) -> bool {
        CurlTransport::probe()
    }

    fn get(&self, url: &str, timeout_seconds: u32) -> Result<Response, String> {
        let mut command = Command::new("curl");
        command
            .args(["--silent", "--show-error", "--location"])
            .args(["--max-time", &timeout_seconds.to_string()])
            .args(["--user-agent", &user_agent()])
            // The status code is appended on its own final line, which is the
            // simplest way to get it out of curl without parsing headers.
            .args(["--write-out", "\n%{http_code}"])
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = process::without_console(&mut command)
            .output()
            .map_err(|e| format!("Could not run curl: {e}"))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr);
            let message = message.trim();
            return Err(if message.is_empty() {
                format!("The request to {url} failed")
            } else {
                message.to_string()
            });
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let (body, status) = split_trailing_status(&text)
            .ok_or_else(|| "curl did not report a status code".to_string())?;
        Ok(Response {
            status,
            body: body.to_string(),
        })
    }
}

/// Split curl's output into the body and the status code written after it.
fn split_trailing_status(text: &str) -> Option<(&str, u16)> {
    let (body, status) = text.rsplit_once('\n')?;
    Some((body, status.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::split_trailing_status;

    #[test]
    fn the_status_code_is_taken_off_the_end() {
        let (body, status) = split_trailing_status("{\"apps\":[]}\n200").unwrap();
        assert_eq!(body, "{\"apps\":[]}");
        assert_eq!(status, 200);
    }

    /// A body containing newlines must survive intact; only the last line is
    /// the status.
    #[test]
    fn a_multi_line_body_is_kept_whole() {
        let (body, status) = split_trailing_status("first\nsecond\n404").unwrap();
        assert_eq!(body, "first\nsecond");
        assert_eq!(status, 404);
    }

    #[test]
    fn output_without_a_status_is_rejected() {
        assert!(split_trailing_status("no status here").is_none());
        assert!(split_trailing_status("body\nnot-a-number").is_none());
    }
}
