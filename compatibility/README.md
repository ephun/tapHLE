# Compatibility data and tools

This directory holds compatibility *data* and the offline tools that check it.

**The protocol lives in [`docs/compatibility.md`](../docs/compatibility.md)** —
what a result means, how an artifact is identified, the availability policy, the
rating scale, how a report is submitted, and the branch workflow. Read that
first. This file covers only what is in this directory.

## What is here

| Path | What it is |
| --- | --- |
| `apps/*.json` | **Legacy** records, predating the live database. Readable and checkable; do not add to them. |
| `schema-v1.json` | The shape of those legacy records |
| `notes/<app-slug>.md` | Per-app work notes. Continuation aids, **not** compatibility claims. |
| `clickmaps/<app-slug>.json` | Replayable routes from launch to a rating milestone |
| `clickmaps/protocol.md` | The clickmap format and its rules |
| `clickmaps/schema.json` | Clickmap JSON Schema (2020-12) |

New results go to the live database at
<https://taphle.ephun.net/compatibility>, not to files here.

## Checking the legacy records

Run from the repository root:

```powershell
python .\dev-scripts\compatibility.py list
python .\dev-scripts\compatibility.py show ricky
python .\dev-scripts\compatibility.py check
python .\dev-scripts\compatibility.py check --baseline-ref origin/trunk
```

`check` validates exact identities, canonical URLs, hashes, report ordering, and
that every report's tapHLE commit exists and is an ancestor of `HEAD`, without
accessing the network. With `--baseline-ref`, it also proves that existing
reports are an unchanged prefix of the new report list.

The offline validator rejects a report dated after the record's most recent
availability check, and rejects any mismatch between a record's version identity
and its Archive.org source.

### What a legacy record identifies

An exact version is identified by `CFBundleIdentifier` (`bundle_identifier`),
`CFBundleVersion` (`bundle_version`), `CFBundleShortVersionString` when the IPA
contains one (`short_version`), and `MinimumOSVersion` (`minimum_os_version`).

Every source also contains the exact Archive.org item identifier, its canonical
`https://archive.org/details/<identifier>` URL, and the exact IPA filename and
hashes. Multiple original filenames may be listed only when each has been
verified; exactly one is marked as the tested file.

`booted` records whether the application lifecycle began, independently of the
overall status. Feature values remain `unknown` until they were actually
exercised.

Reports are immutable and append-only. Never rewrite a previous observation
because a later commit works better — append one with `supersedes` and explain
it.

## Never commit

An IPA, extracted app, asset, decryption key, save data, personal path, or raw
tapHLE log. Summarize only the minimum diagnostic facts needed for the report.
Keep local apps in the ignored `tapHLE_apps` directory or another private path.
