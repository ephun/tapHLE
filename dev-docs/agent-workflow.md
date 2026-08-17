# Agent workflow for game compatibility

This guide expands the contribution loop in `AGENTS.md`. It is optimized for a
maintainer who names a game and supported host and wants useful iteration
quickly.

Use `app-debugging-playbook.md` for the efficient runtime/static-analysis
protocol. If `app-notes/<app-slug>.md` exists, read it first and continue from
its highest proven milestone rather than rediscovering the same facts.

Read `dev-docs/agent-capability-log.md` before choosing an agent. Its dated,
task-specific observations are evidence rather than a standing ban: give every
agent a narrow, independently reviewable task, then review and retest its work
on the claimed host before relying on it as implementation or compatibility
evidence.

## 1. Capture a reproducible case

Record these facts before diagnosing:

| Field | Example |
| --- | --- |
| Game identity | Title, version, regional/build identifier |
| tapHLE identity | Release or Git commit |
| Host | Windows version/CPU/GPU/driver, or iOS device and OS version |
| Launch | Exact path form and tapHLE options |
| Progress | Last screen, sound, input, or log event that works |
| Failure | Crash, hang, rendering defect, missing input, or wrong behavior |
| Expected result | The next observable behavior that should occur |

Keep a baseline log. Sanitize usernames, local paths, tokens, and proprietary
data before sharing it. Never add the game or its assets to the repository.

For an app selected as an ongoing compatibility target, create or continue
`compat/<app-slug>` (for example, `compat/ricky`). Checkpoint commits are
allowed there. Follow `compatibility/README.md` for exact Archive.org source,
content-hash, app metadata, availability, and database rules. Do not record a
dirty-worktree observation as a compatibility result.

If the game is not available to the agent, ask the maintainer for a log or a
small observation that distinguishes competing hypotheses. Continue with
source inspection or a synthetic TestApp case when that can answer part of the
question.

## 2. Localize the failure

Follow the last trustworthy evidence rather than implementing every nearby
stub. Typical boundaries are:

- loader/linker: `bundle`, `mach_o`, `dyld`, and missing-symbol logs;
- CPU/ABI/memory: `cpu`, `abi`, `mem`, and crashes near guest calls;
- Objective-C dispatch: `objc` and unknown class/selector logs;
- framework behavior: the matching module under `src/frameworks`;
- files and preferences: `fs`, `paths`, and Foundation file APIs;
- graphics/input/windowing: `gles`, UIKit views, and `window`;
- audio: `audio`, AudioToolbox, AVFoundation, and OpenAL.

Add temporary diagnostics when useful, but remove noisy or proprietary output
before handoff. Prefer a focused probe over a broad implementation based only
on a guess.

## 3. Choose the shortest honest fix

The project accepts three kinds of fix:

1. A general implementation when the required behavior is clear and small.
2. A partial implementation covering the observed inputs with explicit
   fallback behavior.
3. A game-specific workaround when evidence shows it is the fastest reliable
   route.

For partial or game-specific behavior, state:

- which game/version or input pattern needs it;
- the observation it reproduces;
- how the condition is bounded so other games are unaffected; and
- what evidence would justify replacing it later.

Do not pretend a stub is a complete API. A clear narrow implementation is more
useful than an overbroad claim.

Compatibility work is app-led, but the usual unit of progress is a reusable
emulator capability. Implement the observed contract of one API, ABI, loader,
graphics, input, audio, or filesystem path and use the target app to prove it.
Do not add a batch of unrelated functions that only return success so startup
continues: that can hide incorrect state and move the crash without improving
compatibility. If a no-op is the correct behavior, explain why the caller does
not require state or output and validate that exact call shape.

## 4. Climb the validation ladder

Stop at the highest affordable level and report where you stopped:

1. A unit test next to deterministic logic.
2. A TestApp probe for a guest-visible API or ABI behavior.
3. `cargo test -- --skip test_app` when the custom SDK is unavailable.
4. Full `cargo test` with the SDK and LLVM from `tests/README.md`.
5. A release build and launch of the exact target game on the claimed host.

The fifth level is the only proof that a game compatibility claim is true.
Passing lower levels still provides useful confidence when game access is
awaiting the maintainer.

Keep level-five app runs in a visible emulator window so the maintainer can
watch the agent's progress. Automated frame capture and coordinate-based input
are encouraged for repeatability, but human-observed rendering, interaction,
orientation, and audio remain distinct evidence.

For database entry, level five must name the app build it was earned on, read
from `tapHLE --info`, against a committed tapHLE revision. Full playability is
not required:
a reproducible boot, menu, or in-game milestone can justify promoting the
branch when its remaining blocker is recorded and regression checks pass.

## 5. Leave a continuation-quality handoff

Summarize:

- the user-visible improvement;
- the root cause or best-supported hypothesis;
- files and subsystem changed;
- checks run and their results;
- exact game and host validation performed; and
- the next observation needed if the target still fails.

When a run reaches a rating milestone, record the route that got there as a
clickmap in `dev-docs/clickmaps/`, in the same commit as the report. The route
is fresh at that moment and never will be again, and the next agent — or the
next version of the same app — starts by replaying it instead of rediscovering
it. Replay before exploring, too. See `dev-docs/clickmaps/protocol.md`.

When a verified result changes the app's star rating, submit it to the
compatibility database at <https://taphle.ephun.net/compatibility> through
`POST /api/report`, as described in `AGENTS.md`. Submit at *every* boundary the
app crosses, as it crosses it — carrying two stars forward unreported because
three looks reachable is how a rating goes unrecorded, and a boundary missed
cannot be filled in afterwards. Never edit an earlier report; each one is a
dated snapshot. Do not add records under `compatibility/apps`: those predate the
live database and remain only until they are migrated.

Agent work should make the next iteration cheaper, even when one turn cannot
reach the game menu.

When work remains in progress, update the sanitized app note with proven facts,
rejected hypotheses, dirty diagnostics, and the single next discriminator.
Never copy raw logs, app code/data, screenshots, or personal paths into it.

When committing material work created with an agent, add the attribution
trailer required by `AGENTS.md`. For Codex, the final lines of the commit
message are:

```text
Co-authored-by: OpenAI Codex <codex@openai.com>
```

Keep the human author identity configured by the maintainer; the trailer makes
the collaboration explicit without impersonating that maintainer or rewriting
previously published commits.
