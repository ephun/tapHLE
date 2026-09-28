# Making an app work

This is the whole compatibility path: from "I want this game to run" through
diagnosis and a bounded fix to a defensible rating in the public database. It is
written for both a person driving a coding agent and an agent doing the work.

Techniques — how to read a crash, capture a frame, verify audio — are in
`docs/debugging.md`. This document is about the process and what a claim means.

## What a compatibility result means

A report is evidence that **this exact app build, run by this exact tapHLE
revision, on this host, reached this threshold on this date.** The published
normal-release rating is the latest approved cross-platform summary for that app
version; it is not a separate rating per host.

Every part of that is load-bearing:

- **App build.** Identified from bundle metadata read out of the file that was
  actually run, never from a filename or a page title.
- **tapHLE revision.** A committed one. A dirty worktree cannot produce a claim.
- **Host.** Every report names where its evidence was produced. Detailed views
  may compare platforms, while the default end-user view remains one release
  rating per app version. A run on one host is not evidence that another host
  executed the same route; release qualification therefore uses a per-host
  matrix. See `docs/platforms.md`.
- **Date.** Each report is an immutable dated snapshot, never revised.

## For a non-programmer with a coding agent

You can use your own coding agent to improve an app. You do not need to know how
to program, and you do not need to wait for someone else to take your request.

Agents can make mistakes. You stay in control, and you should read the agent's
summary before you publish anything.

### Before you start

You need:

1. A test environment. Windows is the well-trodden path and what the
   instructions below assume; the other four platforms are release targets but
   are not yet places you can comfortably test an app.
2. A fork or clone of tapHLE.
3. A coding agent that can work in that folder.
4. Lawful access to the exact app version you want to test.

**Never upload an IPA, app files, or a raw log to GitHub.** Keep them outside the
repository. The `runtime/apps` folder is ignored by Git for this reason.

Opening an issue does not promise that another contributor will do the work. It
gives you and your agent a place to record the goal and avoid duplicate work.

### The prompt

Replace the bracketed parts, then paste this into the agent. Keep your filled-in
copy private, because it contains a path on your computer — do not paste it into
a public issue or pull request.

```text
I want to improve tapHLE support for [app title and exact version] on Windows.

Exact Archive.org item URL: [URL, or "none"]
Exact Archive.org IPA file name: [file name, or "none"]
Local IPA path: [path, or "download the exact named original to runtime/apps/"]

Read AGENTS.md and docs/compatibility.md before changing anything.

Create or continue the branch compat/[short-app-name]. Check the compatibility
database and compatibility/notes/ first so you do not repeat old work.

If I gave an Archive.org item, verify the exact canonical item URL and original
filename in the live metadata before opening, inspecting, or running the IPA.
If no local path was supplied, download only that exact original into
runtime/apps/. Record a locally computed SHA-256, then read the app identity with
tapHLE --info before composing any report. Stop if the item or filename differs.
If I wrote "none," do not search for an item and do not make an Archive-linked
database report. Ask me to confirm that I authorize use of my lawful local copy
for this task. Never commit the IPA, extracted files, save data, screenshots,
raw logs, or my private file paths.

Find the first thing that stops the app from working. Make the smallest useful
fix, add a focused test when possible, and run the repository checks. Keep
unfinished experiments on the compat branch. When a checkpoint is clean,
reproducible, tested, and its limitations are recorded, prepare it for merge to
trunk even if the app is not finished. Only add a compatibility database
report after the result works from a clean commit with the exact checked IPA.
If work continues into another session, update the sanitized work note so the
next agent does not repeat the same work.

Use simple status updates. Tell me when you need me to click, play, listen, or
describe what is on screen. Push ordinary committed work as required by
AGENTS.md, but do not force-push, create a release tag, or contact anyone unless
I ask.

Follow the commit-credit rule in AGENTS.md. For OpenAI Codex, add this trailer:
Co-authored-by: OpenAI Codex <codex@openai.com>
If you are not OpenAI Codex, state the real tool identity. Do not invent a name
or email address.
```

### Working with the agent

The agent can build code, read logs, add tests, and make commits. You may still
need to play the app, listen to sound, or describe a screen. Short, exact
answers help.

It is normal for an app to take more than one session. Useful unfinished work
belongs on `compat/<app-slug>`. Another agent can continue from its commits and
work note later.

## Availability and provenance

tapHLE may compatibility-test a build that the maintainer has determined in good
faith to be genuinely unavailable or abandoned when there is no current App Store
market alternative for that build. The database may reference the canonical
Archive.org item and exact IPA filename used for a test.

That is a project scope decision, not a blanket claim that "abandonware" is a
legal category or that every archived copy may be downloaded or redistributed. A
public archive listing alone is not proof of legal status. **Do not use this
policy to substitute for an app that is actively sold or otherwise offered by its
rightsholder. Respect DMCA notices and rightsholder requests.** Stop testing and
alert the maintainer if an item is removed, restricted, disputed, or gains a
current legitimate market alternative.

Re-check current availability before every new report. The maintainer makes the
final project-scope decision. Contributors remain responsible for following the
law that applies to them.

Use the exact Archive.org item URL supplied by the maintainer or reporter. Do not
search for, guess, or grope around for an item. After the live metadata confirms
the exact original filename, download only that file into the gitignored
`runtime/apps/` — not a cache directory of your own, and never one outside the
checkout. If the item or filename does not match, stop; do not use a different
local copy because it looks close.

## Freeze the artifact identity first

**Never guess an app's identity — read it from `tapHLE --info` before you compose
a report.** This is a hard rule, not a preference.

```powershell
.\target\release\tapHLE.exe 'C:\private\Game.ipa' --info
```

Record the bundle identifier, bundle version, short version when present, and
minimum OS version. Do this once per artifact, and re-read it before composing
any report.

The bundle identifier is the field the database matches on, so guessing it
silently creates a duplicate app row that a moderator then has to reject, and the
report cannot be edited afterwards — only superseded. Real identifiers are
routinely not what the app's name suggests: two apps on one 2026-07-26 target
list turned out to be plain `Minecrafted` and `com.eeenmachine.` (with a trailing
dot). Copy the identifier and version out of `--info` output; never infer them
from the app name, the Archive filename, or the developer.

Where the file came from is **provenance, not verification**. Record the source
so someone else can obtain the same artifact, and record a locally computed
SHA-256 so a later run can confirm it is testing the same bytes:

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath '<exact local IPA>'
```

That comparison — this run against an earlier run — is the one worth making.

### Why the download hash gate is gone

The protocol used to require matching a freshly downloaded file's MD5 and SHA-1
against the same Archive.org item's published metadata before the file could be
opened, and marked records `content-hash-verified` on that basis.

That check never established what it appeared to. It compares a file against the
hashes published by the same host that just served it, moments earlier, so it
confirms the download did not corrupt in flight and nothing more. It cannot
detect a wrong or tampered upload, because the metadata would be wrong in the
same way — and if the item were wrong there is no independent copy to validate
against. It also cost real time on every session for a guarantee no report
depended on.

Do not reintroduce the gate, and do not treat a missing or mismatched published
hash as a reason to refuse to test a file.

### Check the binary is decrypted before blaming the emulator

An App Store binary ships FairPlay-encrypted. A dump obtained without decryption
still has `LC_ENCRYPTION_INFO` with `cryptid = 1`, and its `__TEXT` is
ciphertext. tapHLE will load such a slice happily and then execute garbage.

The signature is distinctive and easy to misread as an emulator bug: the last
line logged is `Loading <arch> slice for "<app>"`, and then the process dies or
hangs with **no Rust panic**, no register dump, and nothing on stderr even with
`RUST_BACKTRACE=1`. There is no missing selector to chase, because no guest code
has meaningfully run.

Check `cryptid` before investigating any load-time failure of that shape. From
the IPA, without extracting anything to disk:

```python
import zipfile, struct
d = zipfile.ZipFile(ipa).read(exe_member)
# walk the fat header if magic is 0xcafebabe, then per slice walk the load
# commands and look for LC_ENCRYPTION_INFO (0x21) / _64 (0x2C); the cryptid is
# the u32 at offset 16 in that command.
```

`cryptid = 1` means the artifact cannot be tested, whatever the emulator does.
That is a fact about the file, not a compatibility result: record it as the
reason the app is untestable and move on, rather than filing it as an emulator
limitation. Two apps in one session (JungleZuma, Max Adventure Free) were
misdiagnosed as a shared Mach-O loader bug before this was checked.

Do not duplicate IPAs between run directories. Keep them outside Git and pass
their absolute path to tapHLE.

## Resume from evidence, not from launch

Before building or running anything:

1. Check the current branch, worktree, recent commits, and remote tracking state.
2. Read the app's compatibility record and work note.
3. Identify the highest milestone already reproduced on a committed build.
4. Write down the one next observable boundary. Examples are "menu tap reaches
   level select," "the queue starts," or "the first frame presents."
5. Choose the cheapest observation that distinguishes the live hypotheses.

Do not replay the whole investigation merely to become familiar with it. Start
from the last trustworthy milestone and test only the next frontier unless a code
change could have regressed an earlier boundary.

Keep three small lists while working:

- **Proven:** directly supported by a verified run, static inspection, or a
  deterministic test.
- **Rejected:** hypotheses contradicted by evidence, including why.
- **Next discriminator:** one bounded trace, test, or input that decides what to
  implement next.

### Capture the reproducible case

| Field | Example |
| --- | --- |
| App identity | Title, version, regional/build identifier |
| tapHLE identity | Release or Git commit |
| Host | Windows version/CPU/GPU/driver, or the device and OS version |
| Launch | Exact path form and tapHLE options |
| Progress | Last screen, sound, input, or log event that works |
| Failure | Crash, hang, rendering defect, missing input, or wrong behavior |
| Expected result | The next observable behavior that should occur |

Keep a baseline log. Sanitize usernames, local paths, tokens, and proprietary
data before sharing it. Never add the app or its assets to the repository.

If the app is not available to the agent, ask the maintainer for a log or a small
observation that distinguishes competing hypotheses. Continue with source
inspection or a synthetic TestApp case when that can answer part of the question.

## Follow an evidence ladder

Use the lowest-cost layer that can answer the current question:

1. Existing log or compatibility observation.
2. Source search for the exact symbol, status code, selector, or format.
3. Static inspection of the authorized app to recover a branch condition, call
   target, UI hit box, or faulting symbol.
4. A focused unit or TestApp regression.
5. A bounded diagnostic build and one controlled run.
6. A clean committed release build and exact-artifact milestone run.

Static inspection and runtime tracing complement each other. Static work is often
faster for questions such as "what input advances this screen?" Runtime work is
required for questions such as "does that input reach the next level?" Do not
guess at hidden buttons, timers, formats, or ABI behavior through dozens of blind
runs when one branch condition or structure dump can answer it.

Keep static work equally bounded. Extract only the embedded `Info.plist` and
executable needed to answer the named question into a unique temporary directory;
do not unpack an entire library of apps. Record the summarized condition or
symbol, then remove the exact temporary directory when it is no longer needed.

### Localize the failure

Follow the last trustworthy evidence rather than implementing every nearby stub.
Typical boundaries are:

- loader/linker: `bundle`, `mach_o`, `dyld`, and missing-symbol logs;
- CPU/ABI/memory: `cpu`, `abi`, `mem`, and crashes near guest calls;
- Objective-C dispatch: `objc` and unknown class/selector logs;
- framework behavior: the matching module under `crates/taphle/src/frameworks`;
- files and preferences: `fs`, `paths`, and Foundation file APIs;
- graphics/input/windowing: `gles`, UIKit views, and `window`;
- audio: `audio`, AudioToolbox, AVFoundation, and OpenAL.

`docs/debugging.md` has the technique for each.

## Choose and bound the fix

The project accepts three kinds of fix. Prefer the smallest complete behavior
supported by evidence.

1. **A general implementation** when the required behavior is clear and small.
   It should validate its inputs and fail safely for unsupported variants.
2. **A partial implementation** covering the observed inputs. State the exact
   supported shape and leave a visible fallback or TODO for other routes.
3. **An app-specific workaround** when evidence shows it is the fastest reliable
   route. Gate it as narrowly as possible and explain the observed behavior it
   preserves.

For partial or app-specific behavior, state which app/version or input pattern
needs it, the observation it reproduces, how the condition is bounded so other
apps are unaffected, and what evidence would justify replacing it later.

Compatibility work is app-led, but the usual unit of progress is a reusable
emulator capability. Implement the observed contract of one API, ABI, loader,
graphics, input, audio, or filesystem path and use the target app to prove it.

**Do not pretend a stub is a complete API.** Do not add a batch of unrelated
functions that only return success so startup continues: that can hide incorrect
state and move the crash without improving compatibility. If a no-op is the
correct behavior, explain why the caller does not require state or output and
validate that exact call shape.

Treat a later crash as a new observation, not automatically as progress. A useful
step implements or corrects a coherent guest-visible capability and proves that
the target crosses that boundary. Suppressing several imports with unrelated
success stubs is not a checkpoint; preserve such experiments only as leads and
review each API contract separately.

Do not broaden scope to adjacent formats, platforms, or APIs just because they
are nearby. Add a synthetic regression that contains no proprietary fixture
whenever deterministic logic can be isolated.

## Climb the validation ladder

Stop at the highest affordable level and report where you stopped:

1. A unit test next to deterministic logic.
2. A TestApp probe for a guest-visible API or ABI behavior.
3. `cargo test -- --skip test_app` when the custom SDK is unavailable.
4. Full `cargo test` with the SDK and LLVM from `crates/taphle/tests/README.md`.
5. A release build and launch of the exact target app on the claimed host.

The fifth level is the only proof that a compatibility claim is true. Passing
lower levels still provides useful confidence when app access is awaiting the
maintainer.

Keep level-five runs in a **visible** emulator window so the maintainer can watch
startup, rendering, orientation changes, automated input, crashes, and newly
reached screens. Automated frame capture and coordinate-based input are
encouraged for repeatability, but human-observed rendering, interaction,
orientation, and audio remain distinct evidence. Do not hide a run with
`--headless` or an off-screen window unless the named experiment is genuinely
independent of UIKit, input, and graphics.

## App branches

Work on an app in `compat/<app-slug>`; for example, Ricky work belongs on
`compat/ricky`. Exploratory checkpoint commits are allowed there so
investigations are reproducible and do not depend on a dirty worktree. Push
useful checkpoints to the matching remote branch so another agent can resume
them.

**Never force-push or otherwise rewrite a commit referenced by a compatibility
report.**

Merge or otherwise promote the branch only after:

1. an artifact identified with `tapHLE --info` produces a reproducible
   compatibility milestone on a committed tapHLE revision;
2. the exact achieved state and remaining blocker are appended to the database;
   and
3. the relevant normal regression checks pass.

Meeting these gates defines a stable compatibility checkpoint. Merge it to
`trunk` with its limitations recorded even when more app work remains. Full
playability is not required — a smaller verified milestone is useful when the
database states it honestly. Leave unfinished, unverified, or unstable
experiments on the compatibility branch.

Preserve every commit named by a report when merging: use a fast-forward or merge
commit, not a squash or rebase that removes the tested commit from `HEAD`
history. If history must change, rerun the artifact on a new preserved commit
before recording the result.

## Commit, then make compatibility claims

A dirty-worktree run is a useful experiment but never database evidence.

1. Commit the focused implementation on `compat/<app-slug>`.
2. Build and run the relevant tests from that exact commit.
3. Re-verify the IPA hash and replay the milestone on the claimed host.
4. If the milestone is reproducible, commit its clickmap with the tested
   implementation when the route needs more than launch.
5. Submit the verified result to the compatibility database, referencing that
   exact tested commit.
6. Merge a stable, documented milestone to `trunk` even when known limitations
   remain.

Keep implementation, agent-policy documentation, and compatibility reports in
separable commits. This makes incomplete app experiments easy to continue or
revert without losing durable process improvements.

Crossing a rating-state boundary does two things, not one: the reusable fix graduates to
`trunk` *and* the report goes to the database. Do only the first and a real result
stays invisible; do only the second and the claim cannot be reproduced.

### Threshold-publication closeout

Do not call a new rating complete until all four conditions hold:

1. An artifact whose identity was read with `tapHLE --info` has reproduced the
   milestone on a committed implementation revision.
2. `POST /api/report` has accepted the report for that exact revision. A
   `pending_moderation` response is successful submission; it is not a reason to
   wait for approval before continuing.
3. The compatibility checkpoint has been merged into `trunk` without removing the
   tested revision, and `trunk` has been pushed to `origin`.
4. `git merge-base --is-ancestor <tested-commit> origin/trunk` exits zero after
   the push.

A pushed `compat/<app-slug>` branch is a checkpoint, not threshold completion. If
canonical provenance or the submission token is missing, record that exact
blocker and keep threshold publication open; do not silently leave the report or
the `trunk` promotion undone.

## Compatibility rating states

tapHLE uses ten cumulative states. These states are the rating model; there is
no separate conceptual "state" layered over a numeric rating. The API stores the
ASCII form, and the app and website render it with emoji:

| Stored | Display | Meaning |
| --- | --- | --- |
| `?????` | ❓❓❓❓❓ | No threshold has been established. |
| `*XXXX` | ⭐❌❌❌❌ | Exact execution was observed; meaningful interaction was tested and failed, so every cumulative higher threshold is also known not met. |
| `*????` | ⭐❓❓❓❓ | Exact execution was observed; higher thresholds were not established. |
| `**XXX` | ⭐⭐❌❌❌ | Stable meaningful content and basic interaction work; primary activity was tested and failed. |
| `**???` | ⭐⭐❓❓❓ | Stable meaningful content and basic interaction work; higher thresholds were not established. |
| `***XX` | ⭐⭐⭐❌❌ | Primary activity or core functionality supports meaningful use; end-to-end use was tested and failed. |
| `***??` | ⭐⭐⭐❓❓ | Primary activity or core functionality supports meaningful use; higher thresholds were not established. |
| `****X` | ⭐⭐⭐⭐❌ | The intended experience works end to end without a major blocker; fully working was tested and not met. |
| `****?` | ⭐⭐⭐⭐❓ | The intended experience works end to end without a major blocker; fully working was not established. |
| `*****` | ⭐⭐⭐⭐⭐ | Fully working to the extent reasonably testable. |

A question mark means that threshold is not established. It does not mean
"probably works." An X means the threshold was exercised and is known not met.
Because thresholds are cumulative, only the ten states above are valid; mixed
forms such as `***X?` are not.

Concrete examples:

- An app that launches and sits at a title screen without tested input is
  `*????`, not two stars because it remained alive for a long time.
- A responsive menu is `**???` until the primary activity is exercised.
- A game with a demonstrably usable gameplay loop can reach `***??`; a
  timer, process survival, or changing frames alone cannot establish that.
- If that loop renders mirrored, clipped, or otherwise unusably, the third
  threshold is not met. Record X only when the threshold was actually tested;
  otherwise leave it unknown.
- A human who completes the primary intended experience without a major blocker
  can establish `****?`; five stars require reasonable testing of the remaining
  important behavior.

The database derives a leading-star count for backward-compatible read clients,
but new reports submit the exact compatibility state. Do not turn legacy numeric
ratings, run duration, survival, or a later threshold into guessed X or question
mark positions.

### Source ceilings

- Agents can establish at most three stars. The server enforces this.
- Humans are required for four and five stars. The server enforces this.
- A survey-style script conceptually stops at one star because it does not test
  meaningful interaction. The current API groups it with deterministic test
  harnesses and can technically accept automation evidence through three stars.
  **TODO:** distinguish survey evidence in code before claiming that narrower
  one-star ceiling is enforced.

Agent and meaningful visual automation reports require a screenshot under the
current server policy. Evidence from a deterministic harness may establish only
the thresholds it actually exercises; source ceilings never turn liveness into
interaction or playability.

The scale is adapted from the [touchHLE app
database](https://appdb.touchhle.org/), whose database content is published under
the Creative Commons Attribution 4.0 license. Results from touchHLE or HyperHLE
are useful testing leads — and, since tapHLE is a fork, its lowest-rated apps are
the highest-yield places to look for work. But a lead does not become a tapHLE
rating until the exact app file has been identified with `tapHLE --info` and run
with a committed tapHLE build.

## Recording the result

The [compatibility database](https://taphle.ephun.net/compatibility) is the
public answer to "how well does this app work?" It is a live web application,
**tapHLEdb** — a fork of
[app-compatibility-db](https://github.com/hikari-no-yume/app-compatibility-db) —
self-hosted by the maintainer, with source at
[ephun/tapHLEdb](https://github.com/ephun/tapHLEdb).

A live application is the right shape for this data: it is edited continuously by
humans, coding agents and (later) tapHLE telemetry, which does not fit a
commit-per-edit Git workflow.

What lives where, so the two never duplicate each other:

- **tapHLEdb — the database.** Structured data: an app's identity, its versions,
  and dated reports carrying an exact compatibility state, the tapHLE version,
  the host, the source of the result, and evidence. Each report is a dated
  snapshot and is never revised, so its frontier records where the app stood *at
  that commit*. It answers *"where does this app stand?"*
- **`compatibility/notes/<app>.md` — the notebook.** The debugging narrative:
  evidence, root causes and the next discriminator, kept current. Its frontier is
  where the app stops *now*. It answers *"how do I push this app further?"* It is
  explicitly not a compatibility claim.

The two frontiers are different facts, not duplicates: one is history, one is
present.

### Host, artifact and verification identity

The release rating is cross-platform, while every evidence report is
host-specific. Every new report identifies:

- host platform, architecture and OS version;
- the full tapHLE Git commit;
- the tested binary or package SHA-256 and reproducible build provenance,
  including build profile and toolchain/runner identity;
- the exact app artifact identity: bundle identifier and version fields from
  `tapHLE --info`, plus the lawfully obtained app artifact hash;
- compatibility state and frontier/evidence;
- producer and submitter identity; and
- verification type.

The Git commit is the canonical source identity. A product hash is still required
because it proves which output from that source was installed and run.

`compatibility` is the verification type for ordinary rating history. Submit one
when the exact rating state changes in either direction, following the boundary
rules below.
`release_verification` is a reconfirmation for a named release candidate: it says
an existing rating was reproduced on a particular platform, commit and product.
It does not create a new rating boundary and must remain distinguishable in the
API and UI. Never use a release reconfirmation to reconstruct a boundary that was
missed.

An app/version may be catalogued before testing and then displays `?????`.
Cataloguing establishes identity only; it is not a compatibility result.

### Who records it

- **You tested it yourself.** Sign in at the database with your GitHub account
  and fill in the form. Nothing else is needed, and you never have to ask
  anyone's permission.
- **Your agent did the work.** Your agent records it, not you. It needs an API
  token, so ask for one in a
  [Start work on a game issue](https://github.com/ephun/tapHLE/issues/new?template=game_target.yml)
  and the maintainer will issue one tied to your agent.

The database records what produced each result — a person, an agent, or automatic
reporting — and that is the main reason a rating there can be trusted. **Do not
submit your agent's work under your own name as though you had played the app.**

The agent token lives at `~/.taphledb-token` (on Windows,
`C:\Users\<your-username>\.taphledb-token`) and nowhere else: read it inline as
`$(cat ~/.taphledb-token)` at the moment of use, never echo it, and never copy it
into this repository, a commit message, a work note, or any command whose output
is recorded. That file must stay off GitHub. The credential never raises the
three-star agent limit. Ordinary credentials land pending moderation. An exact
Ethan-controlled credential may be configured as trusted and then approves only
its own transaction immediately; treat it as a publish credential and keep it off
shared machines and CI.

### Submit every boundary, as it is crossed

Record a compatibility report when the exact ten-state rating changes, **in
either direction** — including unknown-to-failed or failed-to-unknown at the same
leading-star count. A rerun that repeats the same state is submitted only as
`release_verification` for a named release candidate, never as another
compatibility boundary.

**Every rating-state boundary gets its own report, at the time it is crossed.**
Do not jump over an intermediate established threshold and file only the final
state. The temptation to skip the first is strongest exactly when the next
boundary looks close — and that is when it gets missed. Each report is a dated
snapshot of one revision, so the series is what shows which commit moved the
app; a missing boundary erases that, and a later regression hunt has nothing to
bisect against.

A boundary passed without a report **cannot be filled in later.** Do not
reconstruct one from a work note, from memory, or from a rerun on a newer
revision: a report asserts that the artifact was run at that revision and rated
then, and a reconstruction cannot honestly assert it. Submit the rating the app
holds now, note the gap in the work note if it is worth knowing, and file every
later boundary on time.

Reports are immutable and append-only. Never rewrite a previous observation
because a later commit works better. If an old report needs a correction, append
one with `supersedes` and explain it.

### The browser report form

The tapHLE app does not choose a rating, store a local rating, or submit a
report. **Report…** opens the GitHub-authenticated tapHLEdb form in the browser.
It uses the deployed versioned `prefill[v]=1` contract when the library has both
bundle identifier and bundle version; malformed identity falls back to the
generic form.

The current client sends these draft fields when available:

- app: `bundle_identifier`, `display_name`;
- version: `bundle_version`, `short_version`, `minimum_os_version`,
  `app_artifact_sha256`;
- report: `platform`, `os_version`, `architecture`, `taphle_commit`,
  `taphle_release`, `artifact_sha256`, `build_provenance`, `build_profile`, and
  `verification_type=compatibility`.

Empty optional values are omitted. On Android the executable hash is omitted
because `current_exe` names the system `app_process`, not the APK or tapHLE
library. On iOS the client currently omits OS version. A non-40-hex build commit
is omitted. The app artifact hash is sent under the version namespace; the form
copies it into the report draft.

The URL deliberately omits the compatibility state/rating, source class and
identity, moderation/trust fields, credentials, screenshot, icon, timestamps,
test-run ID, result, logs, evidence, and frontier. Prefill is untrusted editable
draft input: it submits nothing, creates nothing, and approves nothing. The
signed-in user must select the compatibility state and complete the normal form.
The exact accepted fields and validation rules are owned by
[tapHLEdb's `API.md`](https://github.com/ephun/tapHLEdb/blob/trunk/API.md).

### The agent submission endpoint

```
POST https://taphle.ephun.net/compatibility/api/report
```

The current request shape is documented in tapHLEdb's `API.md`. **Do not
discover the schema by probing the live endpoint**: a successful probe is a real
submission. New writes use the exact `compatibility_state` and `source_class`
fields.

Use `app_id`/`version_id` only after confirming them from `GET /api/apps`, or
send complete `app` and `version` objects. Versions are matched by exact
`CFBundleVersion` and app artifact SHA-256; version identity fields belong in
`version.extra`. A new app also requires an icon, which the browser form leaves
for the user to upload.

New reports require complete app/version provenance and report provenance,
including source class and producer name, host/architecture/OS, full lowercase
40-hex tapHLE commit, product and app SHA-256 hashes, build provenance/profile,
release channel, verification type, test-run ID, UTC timestamp, result, evidence
description, and visual-output classification. Crashes require crash evidence.
The API binds source identity to the credential; token clients cannot claim
`human`. `release_verification` has additional release-candidate requirements.
Use the live `API.md` rather than copying its evolving field list into scripts.

A rejected transaction writes nothing. That is not permission to discover the
schema by probing; validate against the source contract and its tests first.

### Screenshots

A screenshot of the milestone is welcome and makes the rating easier to confirm
or overturn. Prefer an OS-level capture of the real visible tapHLE/app window over
tapHLE's own frame capture, for the reason in "A frame capture is not necessarily
the screen" in `docs/debugging.md`.

Capture only the tapHLE/app window or a tightly cropped relevant area. Do not
submit the full private desktop when the app window can prove the same fact.
Inspect the final crop for notifications, account names, file paths, unrelated
windows and other sensitive information before attaching it. Never alter the app
content to make evidence look better.

The server currently requires a screenshot for agent reports and for automated
reports when meaningful visual output exists. Human policy may permit omission.
If no safe screenshot can satisfy the applicable source policy, the report is
not ready; never substitute a description of a screen you did not inspect.

### Choosing what to work on

```
GET https://taphle.ephun.net/compatibility/api/apps
```

No credential is needed. The lowest-rated apps need the most help, and an app
listed there with no `compat/<slug>` branch is unclaimed work an agent may start
without being asked.

Apps at `*????` are the cheapest known ratings in the queue, but
survival alone must not be promoted to a higher threshold.

For broad framework work across a large collection, see "Choosing what to work
on" in `docs/development.md`.

## Click maps

A successful click path is evidence. Record it as a **clickmap**: the ordered
list of taps that navigates the app from launch to its current frontier.

`compatibility/clickmaps/` holds these as JSON that `dev-scripts/clickmap.ps1`
can replay, so re-verification is a command rather than a reading exercise, and a
regression check names the step it stopped on. The format and its rules are in
`compatibility/clickmaps/protocol.md`.

**Replay an existing map before working out a route by hand**, including the map
for a different version of the same app. Explore from where the replay stops.

Record one for every app that needs more than a launch to reach its milestone,
and keep it current as the frontier advances. When a run reaches a rating
milestone, put the route in the exact tested commit before submitting the report
that cites that commit — the route is fresh at that moment and never will be
again.

Each step records the screen it starts on, the client-area tap coordinate, and
the screen it leads to. Client coordinates are only meaningful against a stated
**window size and orientation**, because tapHLE maps a client tap into app space
through the viewport and rotation — a landscape-native 1024×768 window, a
portrait 768×1024 window, and a scaled window all map the same client point
differently. So the header states the exact launch options (e.g.
`--landscape-native`), the resulting window size, and the menu-load wait before
the first tap. Prefer coordinates that hit the centre of a generous target so
small layout differences do not miss. When a tap is timing-sensitive, note the
wait after it.

Reuse a proven input recipe. Change one step at the frontier rather than
inventing a new route on every launch.

## Work notes

Create `compatibility/notes/<app-slug>.md` on the app branch when work will span
more than one focused turn. A work note is a **continuation aid, not a
compatibility claim.**

```markdown
# <App> compatibility work note

- Branch and last pushed commit:
- Canonical artifact URL, filename, SHA-256, bundle ID/version:
- Highest clean committed milestone:
- Click map (launch options, window size/orientation, menu-load wait, then the
  ordered `screen -> client (x,y) -> resulting screen` taps):
- Proven facts:
- Rejected hypotheses:
- Current uncommitted diagnostics or code:
- Checks already run:
- Known risks/regressions to watch:
- Next discriminator or implementation step:
```

**Do not put app code, decompiled listings, assets, screenshots, raw logs, keys,
or personal paths in the note.** Exact public provenance, hashes, symbol names,
addresses, API shapes, and summarized behavior are sufficient for continuity.

When a durable fact in a note becomes a test, a database report, or a code
comment, the note no longer needs to carry it. Retire a note when its app's work
is genuinely finished; Git history keeps it.

## A liveness check is not a regression check

Two three-star apps were broken during a single session and neither was caught by
the routine sweep. Both misses were the same mistake in different clothes.

**SPY mouse HD** stayed up for roughly forty seconds and then aborted. A sweep
that launches an app, waits twenty seconds and confirms the process is alive saw a
healthy splash screen and reported success. The app never reached a level again.

**JellyCar 2** was not in the sweep at all. It had been rated three stars weeks
earlier, so it was assumed to still work — and it had been dead for a dozen
commits before anyone launched it.

The complete regression-prevention gate the project needs is:

1. **Check every app that carries a rating**, not a convenient subset. An app
   absent from the sweep is an app whose rating is a claim about the past.
2. **Drive each one to the milestone it is rated for.** Three stars means a
   gameplay loop, so the check has to reach gameplay. Confirming the process is
   alive confirms only that it has not crashed *yet*.
3. **Wait long enough.** Give an app at least as long as its recorded startup time
   plus a margin. Failures that arrive at forty seconds are invisible to a
   twenty-second check.
4. **Compare frames by hash, not by eye or by file size.** Two captures of a
   static screen are byte-identical; two captures of a running app are not. That
   single check distinguishes "playing" from "frozen on a plausible screenshot",
   and it is the check that has caught the most.

That complete gate is **TBD**. `dev-scripts/regression-sweep.ps1` is the current
partial helper: it does 3 and 4 for every app present in the directory passed to
`-AppsDir`, takes per-app launch options from `runtime/default_options.txt`, and
exits non-zero if an unexpected app fails to start or dies. Run it against the
current checkout layout as:

```powershell
.\dev-scripts\regression-sweep.ps1 -AppsDir runtime\apps
```

It does not know which database-rated apps are absent from that local directory,
and known failures in `dev-scripts/regression-sweep-known-bad.txt` do not make its
exit status fail. It therefore does not implement 1 or protect the published
rating set by itself.

**Run it before merging anything that touches a shared path, and after.** Writing
the check by hand instead is how it shrank to eight apps and a twenty-second wait
the last time. One app's screenshot is a sample, not a check.

It also does **not** do 2. Nothing in it leaves the title screen, so it reports
`STATIC` for an app that is merely waiting for input and for one that is wedged,
and cannot tell them apart. Reaching a rated milestone still means following that
app's click map.

Treat `STATIC` as a prompt to look, not as a verdict. Some title screens change
very slowly — Warlords HD's takes about fifteen seconds — so the sweep samples
several frames before calling anything still, and even then Flight Control HD has
read `MOVING` on one run and `STATIC` on the next. That is why `STATIC` does not
fail the run. A genuine freeze is confirmed by driving the app, not by this
number.

**Run it on a machine nobody is using.** Every app appears on the real desktop and
accepts real input, for around half an hour. JellyCar 2 was recorded as `MOVING`
on one run because the maintainer picked it up and played it, when its title
screen does not move on its own. The sweep checks how long the machine has been
idle and marks any such result `MOVING?`, but it can only report the doubt, not
remove it.

### Bisect rather than guess when a regression appears

An app that used to work and now does not has a first bad commit, and finding it
is cheap next to reasoning about which change "looks risky". JellyCar 2 was
pinned in three builds — working at its rated commit, working at the parent of the
suspect merge, dead at the merge — which named the cause with no argument about
it.

The rated commit is recorded in every work note precisely so this is possible.
Use it.

A bisect step script must be **self-contained**. One that calls into
`dev-scripts/` silently "passes" on commits predating the script; verify both
ends of the range before trusting the result.

### The fix for a regression is rarely "revert" or "keep"

The layout-on-mount change that broke JellyCar 2 was also what got Tap Tap
Revenge 2 into gameplay, so both reverting and keeping it cost a three-star
result. Two narrower attempts failed — dropping the recursion into subviews, and
restricting it to `CAEAGLLayer`-backed views — and their failure is what showed
the problem was *when* the layout ran rather than *which views* it touched.

The answer was a condition neither app suggested on its own: lay out on mount
only once launching has finished. When two apps disagree, look for the
distinction that explains both, and treat a narrowing that fails as evidence
about the shape of the bug rather than a dead end.

## Send the work back

Ask the agent to run the checks in `AGENTS.md` and explain what was really
tested. Then open a pull request from the compatibility branch.

A good pull request says:

- what changed;
- the exact app version tested;
- what now works;
- what still does not work;
- which checks passed; and
- which coding agent helped.

Partial progress is welcome when it is clearly described. **Do not claim that an
app or feature works until it was tested on the claimed host from the commit in
the pull request.**

## Historical repository records and work-note follow-up

The three `compatibility/apps/*.json` files and `compatibility/schema-v1.json`
are frozen historical artifacts from the pre-tapHLEdb workflow. Do not add,
edit, or use them as the current reporting schema. `compatibility/README.md`
documents their limited offline validation role.

`compatibility/notes/` remains the active continuation notebook during this
pass. tapHLEdb now has app/version/report-scoped developer notes, which is the
follow-up migration target. Do not partially move notes: the maintainer still
needs to decide scope, target level, historical attribution, and when the
repository copies become read-only or are retired. Until then, keep each note in
its current location and do not duplicate individual entries into tapHLEdb.
