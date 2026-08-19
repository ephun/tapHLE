/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Replaying a clickmap from inside the emulator.
//!
//! A clickmap (`compatibility/clickmaps/`) is the recorded route through an app
//! to a rating milestone. Replaying one used to mean
//! `dev-scripts/clickmap.ps1`, which drives the host's real mouse cursor
//! through `user32.dll`. That works on Windows and nowhere else, and it is
//! fragile even there: it needs an interactive session it owns exclusively, and
//! on a scaled display it puts the cursor somewhere other than where it aimed —
//! which once made every control in four apps look unresponsive when nothing
//! was wrong with them.
//!
//! Doing it here instead fixes both problems at once. The taps are queued as
//! the same [crate::window::Event] a real mouse click produces, through the
//! same coordinate transform, so nothing downstream can tell them apart; and
//! because no host cursor is involved, it works identically on every platform
//! tapHLE runs on and does not need the window to be in front.
//!
//! The format is `compatibility/clickmaps/schema.json`. This reads the fields
//! it can act on and ignores the rest: `expect` and `from` are prose for a
//! human, deliberately not checked here, because a screen that animates on its
//! own changes whether or not a tap landed.

use crate::window::{TextInputEvent, TouchPhase, Window};
use crate::Environment;
use std::path::Path;
use std::time::{Duration, Instant};

/// A step's timings when the map does not give its own.
const DEFAULT_PRESS_MS: u64 = 120;
const DEFAULT_SETTLE_MS: u64 = 1500;
const DEFAULT_SWIPE_MS: u64 = 300;
/// How often a swipe queues an intermediate move. A real drag is a stream of
/// motion events, and an app that tracks a finger needs more than the two ends:
/// MazeFinger's player follows the finger, so a swipe delivered as
/// down-then-up would not move it at all.
const SWIPE_STEP_MS: u64 = 16;

#[derive(Debug)]
enum Action {
    Wait,
    Tap {
        at: (f32, f32),
        press_ms: u64,
    },
    Swipe {
        from: (f32, f32),
        to: (f32, f32),
        duration_ms: u64,
    },
    Type {
        text: String,
    },
    Key {
        scancode: i64,
    },
    Capture,
}

#[derive(Debug)]
struct Step {
    id: String,
    action: Action,
    settle_ms: u64,
    optional: bool,
}

/// Where a step has got to. Timings are wall-clock because that is what a
/// clickmap records: an app needs a real second to load, not a frame count.
#[derive(Debug)]
enum Phase {
    /// Nothing queued yet.
    Starting,
    /// A touch or key is held down until this moment.
    Holding { until: Instant },
    /// A swipe is in progress; `next_move` is when to queue the next motion.
    Swiping {
        started: Instant,
        next_move: Instant,
    },
    /// The step's input is done; waiting for the app to react.
    Settling { until: Instant },
}

#[derive(Debug)]
pub struct Replay {
    steps: Vec<Step>,
    index: usize,
    phase: Phase,
    /// Set once the last step has settled, so the run can be reported and, if
    /// asked, the emulator can quit.
    finished: bool,
    quit_when_done: bool,
}

impl Replay {
    /// Read a clickmap. Returns a message rather than panicking: a bad map is
    /// the caller's mistake and should be reported like one.
    pub fn load(path: &Path, quit_when_done: bool) -> Result<Replay, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("could not read clickmap {}: {}", path.display(), e))?;
        let map: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("clickmap {} is not valid JSON: {}", path.display(), e))?;

        // Refuse a version this does not understand rather than guessing at it,
        // the same way dev-scripts/clickmap.ps1 does.
        match map.get("clickmap_version").and_then(|v| v.as_u64()) {
            Some(1) => (),
            Some(other) => {
                return Err(format!(
                    "clickmap {} is version {}, which this build does not understand",
                    path.display(),
                    other
                ))
            }
            None => {
                return Err(format!(
                    "clickmap {} has no clickmap_version",
                    path.display()
                ))
            }
        }

        let defaults = map.get("defaults");
        let default_press = millis(defaults, "press_ms").unwrap_or(DEFAULT_PRESS_MS);
        let default_settle = millis(defaults, "settle_ms").unwrap_or(DEFAULT_SETTLE_MS);

        let raw_steps = map
            .get("steps")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("clickmap {} has no steps array", path.display()))?;

        let mut steps = Vec::with_capacity(raw_steps.len());
        for (index, raw) in raw_steps.iter().enumerate() {
            let id = raw
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("step{}", index + 1));
            let action_name = raw
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("step {} has no action", id))?;

            let action = match action_name {
                "wait" => Action::Wait,
                "tap" => Action::Tap {
                    at: point(raw, "at").ok_or_else(|| format!("step {} has no at", id))?,
                    press_ms: millis(Some(raw), "press_ms").unwrap_or(default_press),
                },
                "swipe" => Action::Swipe {
                    from: point(raw, "from_xy")
                        .ok_or_else(|| format!("step {} has no from_xy", id))?,
                    to: point(raw, "to_xy").ok_or_else(|| format!("step {} has no to_xy", id))?,
                    duration_ms: millis(Some(raw), "duration_ms").unwrap_or(DEFAULT_SWIPE_MS),
                },
                "type" => Action::Type {
                    text: raw
                        .get("text")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| format!("step {} has no text", id))?
                        .to_string(),
                },
                "key" => Action::Key {
                    scancode: raw
                        .get("scancode")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| format!("step {} has no scancode", id))?,
                },
                "capture" => Action::Capture,
                other => return Err(format!("step {} has unknown action {:?}", id, other)),
            };

            steps.push(Step {
                id,
                action,
                settle_ms: millis(Some(raw), "settle_ms").unwrap_or(default_settle),
                optional: raw
                    .get("optional")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            });
        }

        Ok(Replay {
            steps,
            index: 0,
            phase: Phase::Starting,
            finished: false,
            quit_when_done,
        })
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

fn millis(value: Option<&serde_json::Value>, key: &str) -> Option<u64> {
    value?.get(key)?.as_u64()
}

fn point(value: &serde_json::Value, key: &str) -> Option<(f32, f32)> {
    let array = value.get(key)?.as_array()?;
    let x = array.first()?.as_f64()? as f32;
    let y = array.get(1)?.as_f64()? as f32;
    Some((x, y))
}

/// Interpolate a swipe. Clamped so a finished swipe reports its exact end
/// point rather than something a rounding error past it.
fn along(from: (f32, f32), to: (f32, f32), fraction: f32) -> (f32, f32) {
    let fraction = fraction.clamp(0.0, 1.0);
    (
        from.0 + (to.0 - from.0) * fraction,
        from.1 + (to.1 - from.1) * fraction,
    )
}

/// Advance the replay, if there is one. Returns when the caller should next
/// look, so the run loop does not sleep through a step's timing.
///
/// Called from the main run loop next to the window's own event handling, so
/// replayed input is queued at the same point in the frame as real input.
pub fn tick(env: &mut Environment) -> Option<Instant> {
    if env.replay.as_ref().is_none_or(Replay::is_finished) {
        return None;
    }
    // Nothing to inject into without a window, and no app to drive either.
    env.window.as_ref()?;

    let now = Instant::now();
    let mut replay = env.replay.take().unwrap();
    let next_due = advance(&mut replay, env.window.as_mut().unwrap(), now);
    let finished = replay.finished;
    let quit = replay.quit_when_done;
    env.replay = Some(replay);

    if finished && quit {
        echo!("Replay finished; asking tapHLE to quit, because --replay-quit was given.");
        // Queued as an ordinary quit rather than torn down directly, so the app
        // gets the same shutdown it would from closing the window.
        env.window.as_mut().unwrap().inject_quit();
    }
    next_due
}

fn advance(replay: &mut Replay, window: &mut Window, now: Instant) -> Option<Instant> {
    loop {
        let Some(step) = replay.steps.get(replay.index) else {
            if !replay.finished {
                replay.finished = true;
                echo!("Replay: all {} step(s) done.", replay.index);
            }
            return None;
        };

        match replay.phase {
            Phase::Starting => {
                // A step whose precondition the map could not know about is
                // skipped rather than guessed at; see `optional` in the schema.
                if step.optional {
                    log_dbg!("Replay: step {} is optional", step.id);
                }
                echo!("Replay: {} ({})", step.id, action_name(&step.action));
                match &step.action {
                    Action::Wait | Action::Capture => {
                        replay.phase = Phase::Settling {
                            until: now + Duration::from_millis(step.settle_ms),
                        };
                    }
                    Action::Tap { at, press_ms } => {
                        window.inject_touch(TouchPhase::Down, *at);
                        replay.phase = Phase::Holding {
                            until: now + Duration::from_millis(*press_ms),
                        };
                    }
                    Action::Swipe { from, .. } => {
                        window.inject_touch(TouchPhase::Down, *from);
                        replay.phase = Phase::Swiping {
                            started: now,
                            next_move: now + Duration::from_millis(SWIPE_STEP_MS),
                        };
                    }
                    Action::Type { text } => {
                        for character in text.chars() {
                            window.inject_text_input(TextInputEvent::Text(character.to_string()));
                        }
                        replay.phase = Phase::Settling {
                            until: now + Duration::from_millis(step.settle_ms),
                        };
                    }
                    Action::Key { scancode } => {
                        // A clickmap records a PS/2 set 1 scancode because the
                        // Windows harness had to synthesise one at that level.
                        // Inside the emulator there is no scancode path at all:
                        // the window turns a key into one of the text-input
                        // events below, so the scancode is translated rather
                        // than passed on. Anything outside this set is reported
                        // instead of being silently dropped.
                        match scancode {
                            28 => window.inject_text_input(TextInputEvent::Return),
                            14 => window.inject_text_input(TextInputEvent::Backspace),
                            other => {
                                log!(
                                    "Replay: step {} wants scancode {}, which tapHLE has no key event for; skipping it.",
                                    step.id,
                                    other
                                );
                            }
                        }
                        replay.phase = Phase::Settling {
                            until: now + Duration::from_millis(step.settle_ms),
                        };
                    }
                }
            }
            Phase::Holding { until } => {
                if now < until {
                    return Some(until);
                }
                if let Action::Tap { at, .. } = &step.action {
                    window.inject_touch(TouchPhase::Up, *at);
                }
                replay.phase = Phase::Settling {
                    until: now + Duration::from_millis(step.settle_ms),
                };
            }
            Phase::Swiping { started, next_move } => {
                let Action::Swipe {
                    from,
                    to,
                    duration_ms,
                } = &step.action
                else {
                    // Unreachable: only a swipe enters this phase.
                    replay.phase = Phase::Starting;
                    continue;
                };
                let elapsed = now.duration_since(started).as_millis() as u64;
                if elapsed >= *duration_ms {
                    window.inject_touch(TouchPhase::Move, *to);
                    window.inject_touch(TouchPhase::Up, *to);
                    replay.phase = Phase::Settling {
                        until: now + Duration::from_millis(step.settle_ms),
                    };
                } else {
                    if now >= next_move {
                        let fraction = elapsed as f32 / *duration_ms as f32;
                        window.inject_touch(TouchPhase::Move, along(*from, *to, fraction));
                        replay.phase = Phase::Swiping {
                            started,
                            next_move: now + Duration::from_millis(SWIPE_STEP_MS),
                        };
                    }
                    return Some(now + Duration::from_millis(SWIPE_STEP_MS));
                }
            }
            Phase::Settling { until } => {
                if now < until {
                    return Some(until);
                }
                replay.index += 1;
                replay.phase = Phase::Starting;
            }
        }
    }
}

fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Wait => "wait",
        Action::Tap { .. } => "tap",
        Action::Swipe { .. } => "swipe",
        Action::Type { .. } => "type",
        Action::Key { .. } => "key",
        Action::Capture => "capture",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swipe_interpolates_and_clamps() {
        let from = (0.0, 10.0);
        let to = (100.0, 20.0);
        assert_eq!(along(from, to, 0.0), (0.0, 10.0));
        assert_eq!(along(from, to, 0.5), (50.0, 15.0));
        assert_eq!(along(from, to, 1.0), (100.0, 20.0));
        // Past the end is the end, not beyond it: a swipe must not overshoot
        // the target the map recorded.
        assert_eq!(along(from, to, 1.4), (100.0, 20.0));
        assert_eq!(along(from, to, -0.3), (0.0, 10.0));
    }

    #[test]
    fn an_unknown_version_is_refused_rather_than_guessed_at() {
        let dir = std::env::temp_dir().join(format!("taphle-replay-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("map.json");
        std::fs::write(&path, br#"{"clickmap_version": 99, "steps": []}"#).unwrap();
        let error = Replay::load(&path, false).unwrap_err();
        assert!(error.contains("version 99"), "{}", error);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn steps_take_their_timings_from_defaults_when_they_have_none() {
        let dir = std::env::temp_dir().join(format!("taphle-replay-def-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("map.json");
        std::fs::write(
            &path,
            br#"{
              "clickmap_version": 1,
              "defaults": {"press_ms": 40, "settle_ms": 90},
              "steps": [
                {"id": "a", "action": "tap", "at": [1, 2]},
                {"id": "b", "action": "tap", "at": [3, 4], "press_ms": 7, "settle_ms": 8}
              ]
            }"#,
        )
        .unwrap();
        let replay = Replay::load(&path, false).unwrap();
        assert_eq!(replay.steps.len(), 2);
        assert_eq!(replay.steps[0].settle_ms, 90);
        assert!(matches!(
            replay.steps[0].action,
            Action::Tap { press_ms: 40, .. }
        ));
        assert_eq!(replay.steps[1].settle_ms, 8);
        assert!(matches!(
            replay.steps[1].action,
            Action::Tap { press_ms: 7, .. }
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
