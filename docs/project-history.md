# Project history

Non-normative records: how tapHLE got here, who contributed what, and what was
tried. **Nothing in this file is a rule.** Current behaviour is described by the
other documents in `docs/`; current policy is in `AGENTS.md`. If this page and a
current-state document disagree, the current-state document is right and this one
is out of date.

## Lineage

tapHLE is a fork of the [touchHLE project](https://github.com/touchHLE/touchHLE).
Upstream deserves credit for the emulator architecture and the substantial
implementation this fork began with; tapHLE has independent goals and
contribution policies.

Inherited code remains copyright the touchHLE project contributors and other
authors identified in the source and bundled notices. tapHLE modifications are
copyright their respective contributors.

An inherited document is **untrusted until revalidated**. Upstream's build guide
describes a broader host and target matrix than tapHLE has, and renaming
`touchHLE` to `tapHLE` in a file does not make its claims true for this project.

## Agent contribution provenance

tapHLE records material coding-agent assistance openly. New agent-created commits
carry the attribution trailer required by `AGENTS.md`.

The following published commits were created during OpenAI Codex-led sessions
before that trailer rule was established. This ledger credits that work without
rewriting Git history, invalidating compatibility records, or disrupting existing
checkouts:

| Commit | Subject | Agent |
| --- | --- | --- |
| `f132867cc7ba8ee2c24ac65a46c94dc26dfa9e9a` | Rebrand touchHLE fork as tapHLE | OpenAI Codex |
| `43096e54ce4d571e0761f54992178d5c5c570354` | Document abandonware compatibility policy | OpenAI Codex |
| `f1a4db8949ea61e1ffe06585bb526bd8ae0e0f99` | Add reproducible app compatibility database | OpenAI Codex |
| `b36ec67a648120b119d2e4a1872a2d27eda805a6` | Ignore Python tooling caches | OpenAI Codex |
| `9812f13d96a7844fdb921300dff88c96e2b26ba6` | Harden compatibility report integrity | OpenAI Codex |
| `9b31d508e648e9bb5d29066c0536f8a2005be9d6` | Resolve app symbols through RTLD_DEFAULT | OpenAI Codex |
| `46dce243448bd88c8d23a80b38feba749633d7e9` | Quarantine Ricky delayed-call allocations | OpenAI Codex |
| `39e09d564641b237d1630d1d127fdeb4e8eb494f` | Record Ricky Windows menu compatibility | OpenAI Codex |
| `4f6337fc3b31aedce12ddc173448c02e576f57a4` | Document efficient app troubleshooting | OpenAI Codex |
| `ff2ed05a2811a02253c096278b9dafa89a7204c4` | Harden agent runtime troubleshooting guidance | OpenAI Codex |
| `a6dcb2411f5169f0e65090b4efd8e8fc58c41dd1` | Clarify Ricky work note checkpoint | OpenAI Codex |
| `fad70bc1f74d1315af8e5fd4471a8bde1e768665` | Support packetized MP3 audio queues | OpenAI Codex |
| `d9bc6a5d02322f1902e14ba4a451e8b3a4ade2ee` | Report frames prepared by audio queues | OpenAI Codex |
| `c31aabcf4d50b5e78156297bd0704f431a774e73` | Record Ricky in-game compatibility | OpenAI Codex |

The merge commits below were created by an agent on 2026-08-13 with a bare
`Merge <branch>` message, before the author noticed that the trailer rule in
`AGENTS.md` covers merges too. Their branch commits carry the trailers; the
merges do not, and are recorded here instead of rewriting published history.

| Commit | Subject | Agent |
| --- | --- | --- |
| `462e583be97a7baf08822a2135de115d4f57060a` | Merge feat/foundation-set-values-for-keys | Claude Code (Opus 5) |
| `c1af2bad7fd94db35dcde3387079abbfc43cd504` | Merge fix/cgrectinset-over-inset | Claude Code (Opus 5) |
| `d21210789a0aadc9c5d90ac80391e8558467651a` | Merge feat/uikit-scroll-view-properties | Claude Code (Opus 5) |
| `c191bec063e58cc5bac405235a1ca2a80ae59025` | Merge docs/changelog-jf-gameplay-batch | Claude Code (Opus 5) |
| `700feb419aeca0ec465715b179eeaffa04171ad7` | Merge feat/default-options-jim-and-frank | Claude Code (Opus 5) |
| `e8bf3a9605b14604ffdaac98431d79af42c3b4b6` | Merge chore/wrap-long-comment-lines | Claude Code (Opus 5) |
| `c540c1d5e23cfce9e14acc07b99ecfecb3418186` | Merge fix/eagl-present-texture-wrap | Claude Code (Opus 5) |
| `d4625b63a4bef3ffba9168ea54eddca058a136f9` | Merge fix/cg-image-glyph-row-order-agreement | Claude Code (Opus 5) |
| `f1abf114d1693c0f609daeaf94ede33418024d91` | Merge fix/foundation-url-request-copying | Claude Code (Opus 5) |
| `58e8c1a6dbe39a55e3da0f5b3ed85d932a5a0bff` | Merge fix/foundation-empty-data-slice | Claude Code (Opus 5) |
| `3693c73cfb006ed371235cfcdca117eb742ea26f` | Merge docs/shot-shot-shoot-rotation | Claude Code (Opus 5) |
| `bc10e335ba67640055a94ad746c5184a8140d1d9` | Merge feat/corefoundation-plist-deep-copy | Claude Code (Opus 5) |
| `00d7d6a38c8bbbc91224aea7d3cc016083b344a9` | Merge fix/coregraphics-geometry-string-whitespace | Claude Code (Opus 5) |

This section records contribution provenance only. The commits and their original
author/committer metadata remain unchanged.

## Agent capability observations

Dated, task-specific records of what coding-agent configurations have actually
accomplished on tapHLE. They exist to help choose an agent and to tell reviewers
how much independent verification a contribution needs.

**This is not a benchmark or a leaderboard**, and it is not a claim that a model
will always behave the same way. Model names, hosted surfaces, prompts, and
implementations change. Add a new dated row rather than rewriting an old result,
and do not infer that one app result proves broad emulator competence.

| Date | Model | Agent surface | Effort | tapHLE result | Current use |
| --- | --- | --- | --- | --- | --- |
| 2026-07-18 | Not recorded | Terra | Not recorded | Maintainer experiment did not independently advance the active compatibility work. | Do not use as the sole debugger or checkpoint authority. A narrow maximum-effort experiment is allowed if its output is independently reviewed and retested. |
| 2026-07-18 | Not recorded | Luna | Not recorded | Maintainer experiment did not independently advance the active compatibility work. | Do not use as the sole debugger or checkpoint authority; treat output as an unverified lead. |
| 2026-07-19 | OpenAI GPT-5 session | Codex | Not recorded | Produced reviewed, tested Windows compatibility checkpoints and continuation documentation across the Ricky, Percy, Fantastic Mr. Fox, and Baby Monkey work. | Suitable as the primary implementation agent, with normal maintainer review and exact-app validation. |
| 2026-07-19 | Google Gemini 3.1 Pro | Antigravity | High | Found a useful lead around an unhandled `write`, but its proposed `EBADF` behavior was incomplete because Baby Monkey was writing to standard error. It did not independently reach a safe checkpoint and also produced unrelated success stubs, contradictory AdSupport handling, a stale app note, and an untested `_dladdr` frontier. | Use for bounded leads or reviewable subtasks only. A stronger agent or human must review the diff and rerun the exact artifact before trusting a result. |
| 2026-07-23 | Terra | Codex | Not recorded | On a bounded SPYmouse task, isolated successive deterministic iPhone OS API blockers and produced a Release-built compatibility checkpoint against the user-confirmed IPA. It did not reach a rating threshold or receive independent review. | Treat as a lead/checkpoint candidate only; require stronger-agent or human review plus an exact rerun before merge or database reporting. |
| 2026-07-24 | OpenAI GPT-5 | Codex | Not recorded | On a bounded Snappers 1.08 task, traced seven deterministic framework blockers, built a release checkpoint, and drove the verified Windows artifact through a completed Level 1 interaction. | Suitable for bounded compatibility implementation; retain normal review and rerun requirements before publication. |
| 2026-07-25 | Google Gemini 3.6 Flash | Antigravity | High | Unable to launch interactive foreground GUI windows or complete verified visual app launches, because AGY's ordinary Windows command worker used a background desktop. On 2026-07-26, the repository harness launched the picker with a live visible window handle, captured its internal frame, clicked Quick Options through the interactive desktop, captured the changed screen, and closed tapHLE. AGY print mode later timed out while asked to run one status command, so model/harness responsiveness remains a separate limitation. | Use `dev-scripts/agy-visible-taphle.ps1` and the harness loop in `docs/debugging.md`. Treat results as unverified unless status, pre/post internal frames, and the exact interaction are independently checked. |

The attribution rules these observations feed into — which trailers a commit
carries, and what to do when a model or effort setting is not exposed — are
normative and live in `AGENTS.md`.

## HyperHLE as a source (assessed 2026-07-19)

HyperHLE trunk contains real OpenGL ES 2.0 and 3.0 backends that tapHLE does not
have. This is directly relevant to apps such as Baby Monkey that request
`EAGLRenderingAPIOpenGLES2`.

It is not a safe drop-in downstream base:

- its ES 2.0 completion commit `df59038` depends on the large ES 3.0 foundation
  in `62a93f1` and earlier graphics work;
- the graphics trees differ by more than 11,000 inserted lines, with many later
  app-specific fixes on top;
- its project metadata and internal crate names still primarily say touchHLE; and
- its trunk includes many unrelated dependencies and product changes that do not
  match tapHLE's host policy.

The working decision is to retain tapHLE's base and use HyperHLE as a source for
dedicated, reviewed ports. Begin from the two commits above, trace all required
parents and later corrective commits, preserve original authorship and license
notices, exclude unrelated surfaces, translate active branding, and validate on
each affected host. Reconsider the base only on a dedicated migration branch
after the full diff and regression surface are small enough to audit.

The first bounded port validates that approach. tapHLE commit `fd543d42` adapts
the smaller native ES 2.0 snapshot from HyperHLE and advances Baby Monkey through
two native Windows ES2 contexts and into its display-loop startup. It
deliberately does not import HyperHLE's later ES3 foundation, unrelated product
changes, or incomplete desktop-GL fallback.

**The ported work is by Бусик**, in HyperHLE commit `d640dd4d`, "Add GLES2Native
backend and shader-based present path". That commit adds
`src/gles/gles2_native.rs`, `src/frameworks/opengles/eagl.rs`, and the
shader-based present path — the material `fd543d42` describes adapting.

### Erratum: the ES 2.0 attribution in `fd543d42` is wrong (recorded 2026-07-22)

`fd543d42`'s message cites HyperHLE
`ec06f12b886a166b220df94d44861a2de78299b3` and carries `Co-authored-by:
Dev-HyperHle`. Both are incorrect:

- `ec06f12b` is "triage-22-coremedia-stub-dylib". It touches
  `src/frameworks/core_media.rs`, `src/dyld/dylib_list.rs` and
  `src/frameworks.rs`, and **no graphics files at all**. It is not the source of
  any ES 2.0 code.
- The trailer does not parse. A blank line separates it from the trailer block,
  so `git interpret-trailers --parse` on `fd543d42` returns only the Codex
  trailer, and GitHub records no co-author for the upstream work. Trailers must
  be contiguous; see the commit-provenance rules in `AGENTS.md`.

The commit cannot be rewritten. It lives on `compat/baby-monkey`, not on `trunk`,
with 54 commits after it — including `f0947bc4`, which is the tapHLE version cited
by the published Baby Monkey report in the compatibility database. Rewriting
`fd543d42` would change that descendant's hash and invalidate a published result,
which `docs/compatibility.md` forbids outright. This page and the provenance
comment in `src/gles/gles2_native.rs` are therefore the correcting record.

Credit for the native ES 2.0 backend belongs to **Бусик**. Any future commit that
extends or re-ports this work should carry a contiguous `Co-authored-by:` trailer
naming that author, and cite `d640dd4d`.

The port is not on `trunk` at all: `src/gles/gles2_native.rs` exists only on
`compat/baby-monkey`. If that work is ever promoted to `trunk`, the promoting
commit is the place to attach the attribution that `fd543d42` failed to record.

HyperHLE hashes do not resolve from a fresh clone: `upstream` is touchHLE, and
these objects only exist locally from an earlier fetch. To verify any citation
here, add the source repository first:

```
git remote add hyperhle https://github.com/HyperHLE/HyperHLE
git fetch hyperhle
```

## The iOS host, withdrawn and resumed

An experimental modern-iOS host was merged to `trunk` on 2026-08-01 and withdrawn
on 2026-08-04. It was half-finished and broken, and nothing on Windows could
build or test it, so it sat on `trunk` as untested code claiming a capability
tapHLE did not have.

That objection was never about iOS being unwanted. The 2026-08-17 direction made
iOS a product target, which changes what the job is — making it runnable — but not
the standard: a branch is still the right home for a host nobody can run, and
`trunk` is still for what works. The work is preserved on `feat/ios-host`.

Groundwork for a modern-iOS implementation came from
[@johnny901901901](https://github.com/johnny901901901/touchHLE), and further
inspiration from the LiveExec32 experimentation by the
[LiveContainer team](https://github.com/LiveContainer/LiveExec32).
