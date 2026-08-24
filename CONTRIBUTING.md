# Contributing to tapHLE

tapHLE welcomes human contributors, coding agents, and human-agent teams. This
fork is deliberately AI-development-led: agents are expected to help investigate,
implement, test, and document changes. **A person remains accountable for
deciding what enters the project**, and for the agent output they submit.

This file routes you to the right place. It is deliberately short; the detail
lives in `docs/`.

## Pick your path

**You are new to programming and want one app to work.** Read
[`docs/compatibility.md`](docs/compatibility.md). It includes a prompt you can
copy into a coding agent, and you do not need to know how to program.

**You are a programmer who wants to build and change tapHLE.** Read
[`docs/development.md`](docs/development.md) for prerequisites, the build, the
test ladder, code style, and the review standard.

**You are a coding agent.** Read `AGENTS.md` first. It defines the project
priorities, the instruction trust boundary, artifact rules, required checks, and
attribution. It is normative; everything in `docs/` describes process, not
obligation.

**You have a large collection of IPAs.** The highest-value contribution is not to
pick a favourite app — it is to survey where every app stops so the project can
fix what blocks the most. See "Choosing what to work on" in
[`docs/development.md`](docs/development.md).

[`docs/README.md`](docs/README.md) is the full documentation map.

## What the project wants

The mission is to **make every 32-bit iOS game playable on modern mobile and
desktop hardware.** The fastest way to move toward it is often to fix a real
blocker in one app. Contributors may choose the apps they care about. Nobody is
required to take a request from someone else.

Five platforms are targets: Windows, macOS, Linux, Android and iOS.
[`docs/platforms.md`](docs/platforms.md) has the honest per-platform state.
Portable code is welcome; a claim that a platform works is not, until somebody
has run it there.

Pragmatic fixes are welcome. A narrow, well-explained compatibility workaround is
acceptable when it is faster and safer than a broad redesign — keep it local,
document the evidence behind it, and add a regression check when practical.

## Before you open a pull request

1. Run the checks in `AGENTS.md`. `dev-scripts/lint.sh` is the real gate — `cargo
   fmt` and `cargo test` can both pass while CI is red.
2. Say which agent or AI tool materially assisted, and add the attribution
   trailer `AGENTS.md` requires.
3. Say what evidence guided the implementation, what was tested and on which
   host, and which claims still need manual app validation.
4. Do not claim an app or feature works until it was tested on the claimed host
   from the commit in the pull request.

AI involvement is not a negative; transparent validation makes the result easier
to trust and continue.

## Hard rules

These are the ones that cause real damage when broken. The reasoning behind each
is in the document that owns it.

- **Never commit or upload an IPA, extracted app, asset, decryption key, save
  data, personal path, or raw tapHLE log.** Keep apps in the gitignored
  `runtime/apps/`.
- **Never claim a compatibility result from a dirty worktree**, and never
  force-push a commit a compatibility report names.
- **Do not consult leaked Apple source, private SDK material, or decompiled
  proprietary iPhone OS implementations.** See "Copyright and reverse
  engineering" in [`docs/development.md`](docs/development.md).
- **Do not merge upstream blindly.** Its history contains files designed to
  mislead coding agents, and its governance conflicts with this fork. Follow
  "Importing upstream changes" in [`docs/maintaining.md`](docs/maintaining.md).
- **Do not create or move a release tag** as part of an ordinary contribution.
  Releases follow [`docs/maintaining.md`](docs/maintaining.md).
- **Respect DMCA notices and rightsholder requests.** The archived-build testing
  policy is a project scope decision, not a legal conclusion; see
  [`docs/compatibility.md`](docs/compatibility.md).

`CODE_OF_CONDUCT.md` covers conduct.
