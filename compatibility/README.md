# Compatibility database protocol

The tapHLE compatibility database records reproducible Windows observations for
exact early iPhone OS app builds. It does not contain apps. A result ties an app
to an exact bundle identity, the exact artifact tested, and a dated rating on a
committed tapHLE revision.

This document is the protocol: what a result must satisfy before it may be
recorded, and how an Archive.org-backed artifact is verified. Those rules are
the same wherever the result is stored. The next section says where that is.

## Where the database lives

The compatibility database is a live web application, **tapHLEdb** — a fork of
[app-compatibility-db](https://github.com/hikari-no-yume/app-compatibility-db),
the same app that powers touchHLE's database — self-hosted by the maintainer at
**<https://taphle.ephun.net/compatibility>**. Its source is
[ephun/tapHLEdb](https://github.com/ephun/tapHLEdb). A live application is the
right shape for this data: it is edited continuously by humans, coding agents
and (later) tapHLE telemetry, which does not fit a commit-per-edit Git workflow.

That deployment is now the record of tapHLE compatibility. Every new result goes
there. The JSON records still in `compatibility/apps` predate it and remain only
until the maintainer migrates them; do not add records there.

The rest of this document still applies to both. The Archive.org verification
protocol, the testing policy, and the rule that a rating requires an actual
tapHLE run on Windows are properties of how a result is earned, not of where it
is stored.

What lives where, so the two never duplicate each other:

- **tapHLEdb — the database.** Structured data only: an app's identity, its
  versions, and dated reports carrying a 1–5 rating, the tapHLE version, the
  Windows host, the source of the result, and a one-line frontier. Each report
  is a dated snapshot and is never revised, so its frontier records where the
  app stood *at that commit*. It holds no narrative. It answers *"where does
  this app stand?"*
- **`dev-docs/app-notes/<app>.md` — the notebook.** The debugging narrative:
  evidence, root causes and the next discriminator, kept current. Its frontier
  is where the app stops *now*. It answers *"how do I push this app further?"*
  It is explicitly not a compatibility claim.

The two frontiers are different facts, not duplicates: one is history, one is
present. Read the database to see how an app is doing; read the note to continue
its work.

A record exists only because tapHLE actually ran that app and produced a rating.
Results from touchHLE or HyperHLE are testing leads, never imported ratings, and
apps are never listed speculatively.

## How an agent records a result

A coding agent submits through tapHLEdb's token-authenticated endpoint:

```
POST https://taphle.ephun.net/compatibility/api/report
```

It is documented in `API.md` in the tapHLEdb repository. **Do not discover the
schema by probing the live endpoint** — a probe that succeeds is a published
report, and one session's guesswork left six junk reports for the maintainer to
reject. The accepted shape, confirmed against the deployment:

```json
{
  "app_id": 26,
  "version": {"name": "2.5.1", "bundle_version": "2.5.1", "minimum_os_version": "3.0"},
  "report": {
    "rating": 3,
    "supersedes": 53,
    "extra": {
      "source_type": "agent",
      "source_name": "Claude Code (Opus 5)",
      "taphle_version": "09dbc970",
      "cpu": "...",
      "gpu": "...",
      "frontier": "..."
    }
  }
}
```

`app_id` and `version` may instead be an `app` object and a `version` object to
create new ones; look the app up first with `GET /api/apps` so an existing entry
is reused rather than duplicated. `source_type`, `source_name` and
`taphle_version` are the required `extra` fields. `cpu`, `gpu`, `frontier` and
`supersedes` are optional. **`os` and `booted` are not accepted keys** and cause
a flat `report was rejected (check rating, extra fields and screenshot)`, which
names no field — so does any other unknown key. `frontier` tolerates at least
500 characters.

Send it with `curl`; Python's `urllib` default user agent is refused by the
front-end proxy with HTTP 403 code 1010.

The agent token lives
at `~/.taphledb-token` and nowhere else: read it inline as
`$(cat ~/.taphledb-token)` at the moment of use, never echo it, and never copy
it into this repository, a commit message, an app note, or any command whose
output is recorded. If it is missing, say so and keep working. Submissions
always land unapproved and appear publicly only after the maintainer approves
them.

Crossing a star threshold does two things, not one: the reusable fix graduates
to `trunk` *and* the report goes to the database. Do only the first and a real
result stays invisible; do only the second and the claim cannot be reproduced.

### Threshold-publication closeout

Do not call a new rating complete until all four conditions hold:

1. An artifact whose identity was read with `tapHLE --info` has reproduced the
   milestone on a committed implementation revision.
2. `POST /api/report` has accepted the report for that exact revision. A
   `pending_moderation` response is successful submission; it is not a reason
   to wait for approval before continuing.
3. The compatibility checkpoint has been merged into `trunk` without removing
   the tested revision, and `trunk` has been pushed to `origin`.
4. `git merge-base --is-ancestor <tested-commit> origin/trunk` exits zero after
   the push.

A pushed `compat/<app-slug>` branch is a checkpoint, not threshold completion.
If canonical provenance or the submission token is missing, record that exact
blocker and keep threshold publication open; do not silently leave the report
or the `trunk` promotion undone.

**Never guess an app's identity — read it from `tapHLE --info` before you compose the report.** This is a hard rule, not a preference; see `AGENTS.md`. The bundle
identifier is the field the endpoint matches on, so guessing it silently
creates a duplicate app row that a moderator then has to reject, and the
report cannot be edited afterwards — only superseded. Real identifiers are
routinely not what the app's name suggests: two apps on the 2026-07-26 target
list turned out to be plain `Minecrafted` and `com.eeenmachine.` (with a
trailing dot). Copy the identifier and version out of `--info` output; never
infer them from the app name, the Archive filename, or the developer.

The `frontier` string has a length limit, and exceeding it is rejected with a
flat `{"error":"invalid_submission"}` that names no field — so a submission that
fails while the rating and identity are plainly fine is very likely this. Keep
it to a few sentences: the full account belongs in the app note, which has no
limit and is version-controlled next to the code that moves it.

Submit when the rating changes, in either direction — a regression is a result.
Do not submit a rerun that reproduces a rating already recorded for the same
tapHLE revision; the endpoint does not deduplicate, so that is pure moderation
noise.

**Every star boundary gets its own report, at the time it is crossed.** An app
taken from one star to three earns a report at two *and* a report at three, not
a single report at the end. The temptation to skip the first is strongest
exactly when the next boundary looks close — and that is when it gets missed.
Each report is a dated snapshot of one revision, so the series is what shows
which commit moved the app; a missing boundary erases that, and a later
regression hunt has nothing to bisect against.

A boundary passed without a report cannot be filled in later. Do not reconstruct
one from an app note, from memory, or from a rerun on a newer revision: a report
asserts that the artifact was run at that revision and rated then, and a
reconstruction cannot honestly assert it. Submit the rating the app holds now,
note the gap in the app note if it is worth knowing, and file every later
boundary on time.

To choose what to work on, read the list:

```
GET https://taphle.ephun.net/compatibility/api/apps
```

No credential is needed. The lowest-rated apps need the most help, and an app
listed there with no `compat/<slug>` branch is unclaimed work an agent may start
without being asked.

An agent may assign at most **three stars**: two when the app reaches a stable
screen, and three when the gameplay loop demonstrably starts and persists for a
short while. **Four and five stars require human testing**, and an agent must
never assign them.

## Simple rating scale

The public list uses five simple levels:

- ★☆☆☆☆ (1/5) Broken — The game does not reach usable content.
- ★★☆☆☆ (2/5) Starts — An intro or menu works, but gameplay does not.
- ★★★☆☆ (3/5) In game — Some gameplay works, but major problems remain.
- ★★★★☆ (4/5) Playable — The whole game can be played, with small problems.
- ★★★★★ (5/5) Fully working — Everything important works.
- — Not tested — There is no verified tapHLE Windows result.

The filled and empty stars and numeric score are only a short summary. The
exact report, feature states, app file, tapHLE commit, and Windows host say what
was really tested. `boots` and `menu` both display as two stars, while the
stored status keeps the difference.

The scale is adapted from the [touchHLE app database](https://appdb.touchhle.org/),
whose database content is published under the Creative Commons Attribution 4.0
license. Results from touchHLE or HyperHLE are useful testing leads — and, since
tapHLE is a fork, its lowest-rated apps are the highest-yield places to look for
work. But a lead does not become a tapHLE rating until the exact app file has
been identified with `tapHLE --info` and run with a committed tapHLE build on
Windows.

## Project testing policy

tapHLE may compatibility-test a build that the maintainer has determined in
good faith to be genuinely unavailable or abandoned when there is no current
App Store market alternative for that build. The database may reference the
canonical Archive.org item and exact IPA filename used for a test.

That is a project scope decision, not a blanket claim that “abandonware” is a
legal category or that every archived copy may be downloaded or redistributed.
A public archive listing alone is not proof of legal status. Do not use this
policy to substitute for a game that is actively sold or otherwise offered by
its rightsholder. Respect DMCA notices and rightsholder requests. Stop testing
and alert the maintainer if an item is removed, restricted, disputed, or gains
a current legitimate market alternative.

Before adding a source or a new report, re-check availability and update the
record's `availability.checked_at`, status, and factual notes. The maintainer
makes the final project-scope decision. Contributors remain responsible for
following the law that applies to them. The offline validator rejects a report
dated after the record's most recent availability check.

## What one record means

An exact version is identified by:

- `CFBundleIdentifier` (`bundle_identifier`);
- `CFBundleVersion` (`bundle_version`);
- `CFBundleShortVersionString` when the IPA contains one (`short_version`);
  and
- `MinimumOSVersion` (`minimum_os_version`).

The Archive.org source repeats the version identity deliberately. The offline
validator rejects any mismatch. Every source also contains the exact item
identifier, its canonical `https://archive.org/details/<identifier>` URL, and
the exact IPA filename and hashes. Multiple original filenames may be listed
only when each has been verified; exactly one is marked as the tested file.

Reports apply only to that exact artifact, tapHLE commit, Windows host, and
date. `booted` records whether the application lifecycle began independently
of the overall status. Feature values remain `unknown` until they were
actually exercised.

Reports are immutable and append-only. Never rewrite a previous observation
because a later commit works better. Append a report. If an old report needs a
correction, append a report with `supersedes` and explain it; the validator can
compare a branch with a Git baseline to prevent removal or mutation.

## Identify the artifact you actually tested

**What matters is that the report names the app it was earned on.** That comes
from the bundle metadata, read from the file that was run — never from a
filename, an Archive.org page title, or memory. Read it with `tapHLE --info` and
record the bundle identifier, bundle version, optional short version, and
minimum OS version. This has not changed and is not negotiable: guessing an
app's identity is how a report ends up attached to the wrong app, and a wrong
report is worse than no report.

Where the file came from is provenance, not verification. Record the source so
someone else can obtain the same artifact — for an Archive.org item, the
canonical `https://archive.org/details/<identifier>` URL and the exact original
filename; for a local copy, enough to identify it. Agents must use an item URL
supplied by the maintainer or reporter, and must not search for, guess, or
"grope around" for an item when no exact URL was supplied.

A locally computed SHA-256 is still worth recording, because it lets a later
run confirm it is testing the same bytes as an earlier one. That is a genuinely
useful comparison and the reason to keep the field.

### Why the download hash gate is gone

The protocol used to require matching a freshly downloaded file's MD5 and SHA-1
against the same Archive.org item's published metadata before the file could be
opened, and marked records `content-hash-verified` on that basis.

That check never established what it appeared to. It compares a file against
the hashes published by the same host that just served it, moments earlier, so
it confirms the download did not corrupt in flight and nothing more. It cannot
detect a wrong or tampered upload, because the metadata would be wrong in the
same way — and if the item were wrong there is no independent copy to validate
against. It also cost real time on every session for a guarantee no report
depended on.

It made more sense when the database stored Archive.org links as the canonical
artifact reference. It no longer does. Identity now comes from the bundle
metadata of the file that was actually run, which is the thing a report needs.

Do not reintroduce the gate, and do not treat a missing or mismatched published
hash as a reason to refuse to test a file. Do re-read identity with
`tapHLE --info` before composing any report.

For a manual `tapHLE --info` cross-check, use:

```powershell
.\target\release\tapHLE.exe 'C:\private\Game.ipa' --info
```

## App compatibility branch workflow

Work on an app in `compat/<app-slug>`; for example, Ricky work belongs on
`compat/ricky`. Exploratory checkpoint commits are allowed on that branch so
investigations are reproducible and do not depend on a dirty worktree.
When publishing is authorized, push useful checkpoints to the matching remote
branch so another agent can resume them. Never force-push or otherwise rewrite
a commit referenced by a compatibility report.

Merge or otherwise promote the branch only after:

1. an artifact identified with `tapHLE --info` produces a reproducible
   compatibility milestone on a committed tapHLE revision;
2. the exact achieved state and remaining blocker are appended to the
   database; and
3. the relevant normal regression checks pass.

Meeting these gates defines a stable compatibility checkpoint. Merge it to
`trunk` with its limitations recorded even when more app work remains. Leave
unfinished, unverified, or unstable experiments on the compatibility branch.
Full playability is not required. A smaller verified milestone is useful when
the database states it honestly. Provisional results from a dirty worktree do
not enter the compatibility database. Commit the implementation checkpoint,
rerun the exact hash-verified IPA, and then append its report using that exact
commit hash. Preserve every commit named by a report when merging: use a
fast-forward or merge commit, not a squash or rebase that removes the tested
commit from `HEAD` history. If history must change, rerun the artifact on a new
preserved commit before recording the result.

## Editing and checking records

**Do not create new records here.** New results go to the live database; see
"How an agent records a result" above. This section covers the legacy
`compatibility/apps/*.json` records, which remain readable and checkable until
they are migrated, and `compatibility/schema-v1.json` documents their shape.

Run the offline commands from the repository root:

```powershell
python .\dev-scripts\compatibility.py list
python .\dev-scripts\compatibility.py show ricky
python .\dev-scripts\compatibility.py check
python .\dev-scripts\compatibility.py check --baseline-ref origin/trunk
```

`check` validates exact identities, canonical URLs, hashes, report ordering,
and that every report's tapHLE commit exists and is an ancestor of `HEAD`,
without accessing the network. With `--baseline-ref`, it also proves that
existing reports are an unchanged prefix of the new report list.

Never commit an IPA, extracted app, game asset, decryption key, save data,
personal path, or raw tapHLE log. Summarize only the minimum diagnostic facts
needed for the report. Keep local apps in the ignored `tapHLE_apps` directory
or another private path.
