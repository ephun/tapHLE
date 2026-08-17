# Contributing to tapHLE

tapHLE welcomes human contributors, coding agents, and human-agent teams. This
fork is deliberately AI-development-led: agents are expected to help
investigate, implement, test, and document changes. A person remains
accountable for deciding what enters the project.

Read `AGENTS.md` first. It defines the project priorities, trust boundary,
artifact rules, and validation expectations.

If you are new to programming and want to use a coding agent for one game,
start with `HELP_A_GAME.md`. It includes a prompt you can copy.

For selected-game diagnosis, follow `dev-docs/app-debugging-playbook.md` and
read any sanitized continuation note under `dev-docs/app-notes/` before
repeating runtime or static-analysis work.

## What the project wants

The product goal is broad compatibility with early iPhone OS games on the
desktop. The fastest way to move toward it is often to fix a real blocker in
one game. Contributors may choose games they care about. Nobody is required to
take a game request from someone else.

Windows, Linux and macOS are the intended desktop platforms; Windows is where
tapHLE is developed and where compatibility is judged, and Linux has not been
tried yet. `AGENTS.md` has the honest per-platform state, and
`dev-docs/packaging.md` says what each one would need. Portable code is
welcome; a claim that a platform works is not, until somebody has run it there.
Android development is not in scope. Broad framework-completeness projects,
aesthetic rewrites, and speculative abstractions are lower priority than a
working game.

Pragmatic fixes are welcome. If a game needs a narrow workaround, keep it
local, document the evidence behind it, and add a regression check when
practical. A clean general implementation is preferred when it takes similar
effort, but contributors are not required to redesign a subsystem before
shipping a useful compatibility improvement.

## Start with evidence

For a game compatibility report, include:

- exact game title, version, and build when known;
- tapHLE commit or release;
- operating system and version, CPU, and GPU;
- exact launch steps and relevant options;
- the last visible or logged successful behavior;
- expected and actual results; and
- the tapHLE log, with personal paths or data removed.

Do not upload the game, its assets, decryption keys, or raw log. A canonical
Archive.org item reference is accepted only under the exact unavailable-build
and verification protocol in `compatibility/README.md`; do not post guessed
items or use an archive as a substitute for an actively marketed game.

## Have a large app collection? Survey it instead of picking one app

The highest-value thing a contributor with many IPAs can do is **not** to pick a
favourite app and chase it. It is to measure where every app in the collection
stops, so the project can fix the causes that block the most apps.

```
cargo build --release
python dev-scripts/survey.py run --apps "D:\path\to\your ipas"
python dev-scripts/survey.py rank
python dev-scripts/survey.py rank --symbols
```

This launches each app briefly, records where it stopped, and ranks the causes
by how many apps hit them. It is resumable, so a large collection can be left
running overnight and picked up later.

**The result file is yours and must never be committed or uploaded.** It lists
the apps you own. The default path is outside the repository and `.gitignore`
covers the pattern; share the *findings*, not the file. A useful contribution
looks like:

> Surveyed 900 apps. Top causes: 40 want `-[UIAlertView setMessage:]`, 31 die on
> an unbound `UIApplicationLaunchOptionsURLKey`, 22 hit the same assertion at
> `src/objc/messages.rs:402`.

That is an issue anyone can act on, and it discloses nothing about your library.

If you work with a coding agent, hand it the result file and ask it to implement
the top causes as *general* emulator support rather than per-app special cases —
`dev-docs/app-debugging-playbook.md` is the method, and `dev-scripts/` has tools
for turning a fault address into a symbol name or a selector. Ask it to re-run
the survey after each batch: the ranking moves as causes are cleared, and that
loop is the point.

A survey row is a few seconds of unattended running. It is **not** a
compatibility rating and must not be filed as one — nothing in it presses a
button, so an app waiting on a tap looks stuck. Ratings still come from actually
playing the app under `compatibility/README.md`.

### The other half: what the collection asks for

The survey needs a build and runs every app. `dev-scripts/demand.py` needs
neither — it reads the import tables out of each binary and subtracts what
`src/` exports, which takes minutes rather than a night:

```
python dev-scripts/demand.py scan --apps "D:\path\to\your ipas"
python dev-scripts/demand.py todo
python dev-scripts/demand.py app "some game"
```

The two answer different questions. A survey says where an app stopped, which is
what to fix next but reveals nothing about what lies behind it — the remaining
work stays unknown until the last app runs. `demand.py` measures the whole
backlog up front, so it can say which framework is worth starting, how much of
one already exists, and which app is *closest* to running rather than merely
furthest along today.

Its counts are references, not calls: an app that links CoreLocation may never
reach the line that needs it. The same privacy rule applies — the file lists
your collection, it is gitignored, and the findings are what you share.

### Both at once

If you have run both tools over the same collection, `cross` joins them into a
single priority order:

```
python dev-scripts/demand.py cross
```

Every gap is labelled `BLOCKING` when some app's run stopped exactly there,
`reachable` when tapHLE failed to bind it at load but something else stopped the
app, and `latent` when nothing has reached it yet. That separates work that is
proven to block from work that merely looks large — a symbol with high demand
and no blocking evidence is behind the current frontier, not in front of it.

Counts are a floor. A survey older than your working tree under-reports, because
anything implemented since has already been dropped from the static gap list.

## Compatibility records and app branches

`compatibility/README.md` is the canonical protocol. The maintainer may accept
good-faith testing of a genuinely unavailable or abandoned build when there
is no current App Store market alternative. This is a project scope decision,
not a blanket legal conclusion about "abandonware." Respect DMCA notices and
rightsholder requests, and re-check current availability before every new
report.

Use the exact Archive.org item URL supplied by the maintainer or reporter; do
not search for or guess one. Verify its canonical identifier and exact IPA
filename against the Archive metadata endpoint, then download only that exact
original into the gitignored `tapHLE_apps/` — not a cache directory of your own,
and never one outside the checkout. Record a locally computed SHA-256 so a later
run can confirm it tested the same bytes; matching a fresh download against the
same host's published hashes is not required and gates nothing. Read the app's
identity with `tapHLE --info` before composing any report — a report may claim a
result only for an artifact identified that way on a committed tapHLE revision.
Append reports; never overwrite an earlier result.

App work belongs on `compat/<app-slug>` (for example, `compat/ricky`).
Exploratory checkpoint commits are allowed there. Keep unfinished, unverified,
or unstable experiments on that branch. Merge a stable checkpoint to `trunk`
once the exact `--info`-identified app reproduces a useful milestone, the
database honestly records what works and what remains, and normal regression
checks pass. Full playability is not required. Dirty-worktree observations are
provisional and must not enter the database.

## Development workflow

1. Create a focused branch from `trunk`; use `compat/<app-slug>` for app work.
2. Initialize submodules with `git submodule update --init --recursive`.
3. Reproduce the failure or create a small synthetic probe.
4. Make the smallest complete change that advances the target game.
5. Add or update a focused test when practical.
6. Run the relevant checks from `AGENTS.md`.
7. For verified app testing, submit the exact result to the live compatibility
   database when publication is authorized.
8. Open a GitHub pull request using the repository template.

Pull requests should say which agent or AI tool materially assisted, what
evidence guided the implementation, what was tested on Windows, and which
claims still need manual game validation. AI involvement is not a negative;
transparent validation makes the result easier to trust and continue.

Version bumps, release tags, and Windows packages follow
`dev-docs/releases.md`. Do not create or move a release tag as part of an
ordinary contribution.

## Copyright and reverse engineering

Compatibility work must not compromise the project legally.

- Prefer public API documentation and clean behavioral experiments.
- You may inspect a target game for an authorized compatibility task under
  `compatibility/README.md`, but do not commit or redistribute proprietary
  material.
- Do not consult leaked Apple source, private SDK material, or decompiled
  proprietary iPhone OS implementations.
- Do not copy code merely because it is visible online. Check its license and
  preserve attribution and notices when reuse is compatible.
- Describe non-obvious external sources in the pull request and, when useful,
  in a nearby comment.

These rules govern intentional research and submitted artifacts. The project
does not presume that an agent's opaque training history is knowable; it does
require contributors to review generated code and reject suspicious or
unverifiable copying.

## Upstream changes

Do not merge upstream blindly. Its history contains files designed to mislead
coding agents, and its governance conflicts with this fork. Follow
`dev-docs/upstream-sync.md`, preserve tapHLE policy files, and prefer vetted
cherry-picks for changes that directly help a Windows game.

## Review standard

Review is outcome-focused:

- Does this improve or protect a target Windows game?
- Is the behavior supported by a reproduction, log, probe, or test?
- Is any shortcut bounded and understandable?
- Are proprietary artifacts absent and source provenance acceptable?
- Were the relevant checks actually run?

Small follow-up improvements are preferable to holding a working,
well-contained fix for an unrelated cleanup.
