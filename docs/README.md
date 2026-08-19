# tapHLE documentation

Start with the row that matches what you are trying to do.

| I want to… | Read |
| --- | --- |
| Run apps in tapHLE | [`user-guide.md`](user-guide.md) |
| Know whether my platform works | [`platforms.md`](platforms.md) |
| Build tapHLE and submit a change | [`development.md`](development.md) |
| Make one specific app work | [`compatibility.md`](compatibility.md) |
| Diagnose a failure I have already localized | [`debugging.md`](debugging.md) |
| Understand how tapHLE is put together | [`architecture.md`](architecture.md) |
| Cut a release, package a build, sync upstream | [`maintaining.md`](maintaining.md) |
| Know how the project got here | [`project-history.md`](project-history.md) |

Outside this directory: `AGENTS.md` is the normative contract for coding agents,
`CONTRIBUTING.md` is the human contributor gateway, and `CHANGELOG.md` records
what changed in each version.

## Who owns which fact

Every durable fact has **one** canonical owner. Other documents link to that
owner instead of restating the fact. This table is what makes that checkable.

| Fact | Canonical owner | Everywhere else |
| --- | --- | --- |
| Platform build/runtime/CI/package/release status | `docs/platforms.md` | Link. **No other file keeps a status matrix.** |
| What "supported", "packaged", "tested" mean | `docs/platforms.md` | Link |
| Release eligibility and the all-five bar | `docs/platforms.md` | `docs/maintaining.md` links for procedure |
| Version numbering, release trigger, tags | `docs/maintaining.md` | Link |
| Packaging per host | `docs/maintaining.md` | Link |
| Upstream sync procedure | `docs/maintaining.md` | Link |
| Build prerequisites and commands | `docs/development.md` | Link |
| The test ladder | `docs/development.md` | Link |
| Code style | `docs/development.md` | Link |
| Copyright and reverse-engineering rules | `docs/development.md` | `AGENTS.md` carries the normative summary |
| TestApp fixture, LLVM and SDK setup | `tests/README.md` | `docs/development.md` links |
| What a compatibility result means | `docs/compatibility.md` | Link |
| Artifact identity and provenance protocol | `docs/compatibility.md` | Link |
| Availability / archive / DMCA policy | `docs/compatibility.md` | Link |
| Rating scale and star thresholds | `docs/compatibility.md` | Link |
| Database submission shape | `docs/compatibility.md` | Link |
| Compatibility branch lifecycle | `docs/compatibility.md` | `AGENTS.md` carries the mandatory summary |
| Click maps | `docs/compatibility.md` | `compatibility/clickmaps/protocol.md` owns the file format |
| Work-note lifecycle and template | `docs/compatibility.md` | Link |
| Legacy JSON records and offline tools | `compatibility/README.md` | `docs/compatibility.md` links |
| Diagnostic technique | `docs/debugging.md` | Link |
| Host harness, input, frame capture | `docs/debugging.md` | Link |
| Subsystem map and code layout | `docs/architecture.md` | Link |
| Frontend design and its rationale | `docs/architecture.md` | Link |
| Rendering handedness, layout behaviour | `docs/architecture.md` | `docs/debugging.md` links |
| CLI options | `OPTIONS_HELP.txt` | It **is** `--help`; never restate an option list |
| Settings precedence | `docs/user-guide.md` | Link |
| Where user files live | `docs/user-guide.md` | Link |
| Agent policy, safety, attribution, required checks | `AGENTS.md` | Link |
| Agent provenance and capability history | `docs/project-history.md` | Never normative |
| What changed in a version | `CHANGELOG.md` | Never a current-state claim |
| App working state | `compatibility/notes/<app>.md` | Explicitly non-canonical |

## Rules for writing tapHLE documentation

**Current state beats roadmap.** Current behaviour and planned behaviour never
share an unqualified sentence. "All five are targets" and "all five work" are
different claims and must read differently.

**Link, do not copy.** If a platform matrix, option list, release rule, or test
command already has an owner above, link to it. The five-platform table existed
in three files at once, and they disagreed.

**Working notes are not documentation.** A work note records hypotheses and next
steps. Compatibility reports, tests, clickmaps and code are the durable record.

**Reference follows implementation.** Any sentence saying "supports", "builds",
"is tested", "requires" or "uses" needs an evidence source: code, CI, a manifest,
a test, a script, or an explicit maintainer decision. If you cannot name one, do
not write the sentence.

**History never defines current behaviour.** `CHANGELOG.md` and
`project-history.md` explain how tapHLE got here, not what works now.

**"Supported" is a commitment word.** It means the maintainer accepts
compatibility claims on that host. It never means the code compiled once.

**Do not add a documentation file.** These eight, plus the root gateways and the
two local READMEs, are the documentation system. Record what you learn in the
file that owns the fact. An agent that invents a document every time it has
something to say leaves a repository full of near-duplicate markdown that nobody
reads. Add a file only when the maintainer asks for one.

**Do not encode a bug diary as process.** If a paragraph would stop being needed
once a regression test prevents the bug, it belongs in the test, not here.
