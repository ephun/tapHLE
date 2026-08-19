# Boundaries crossed but not filed

Every star boundary gets its own report at the time it is crossed
(`compatibility/README.md`). When the endpoint cannot be reached, the report
cannot be filed and the obligation does not go away — so it is written here,
once, instead of being left in seven app notes for somebody to find.

**Rule for whoever picks this up:** submit **the rating each app holds at that
moment, on the revision tested at that moment**. Do not back-date to the
revisions below. A report asserts that an artifact was run and rated at a
revision, and a reconstruction cannot honestly assert that. The revisions are
recorded here so the *series* is visible — which commit moved which app — not
as something to file.

## The blocker

`https://taphle.ephun.net/` answers **HTTP 522** — Cloudflare cannot reach the
origin — at its root and at `/compatibility/api/apps`, on every attempt through
2026-08-18 and 2026-08-19. A plain `GET /api/apps` needs no credential, so this
is not an authentication problem; the token at `~/.taphledb-token` is present
and was never the question. Check the root with one `curl` before composing
anything.

## Open thresholds, oldest first

Identity of every artifact below was read with `tapHLE --info`, not inferred
from a filename.

| App | Bundle | Version | Rating reached | On revision | Note |
| --- | --- | --- | --- | --- | --- |
| Doodle Jump | `com.yourcompany.DoodleJump` | 2.7.1 | ★★★ | `d300e30b` | `doodle-jump.md` |
| Doodle Jump | `com.yourcompany.DoodleJump` | 3.1.1 | ★★ | `b7629c47` | `doodle-jump.md` |
| Doodle Jump | `com.yourcompany.DoodleJump` | 3.4 | ★★ | `b7629c47` | `doodle-jump.md` |
| Doodle Jump HD | `com.limasky.doodlejumpipad` | 1.0 | ★★ | `b7629c47` | `doodle-jump.md` |
| Fieldrunners | `com.subatomicstudios.Fieldrunners` | 1.5.3 | ★★ | `e643eb3e` | `fieldrunners.md` |
| Fieldrunners | `com.subatomicstudios.Fieldrunners` | 1.2.3 | ★★ | `d387265f` | `fieldrunners.md` |
| FallDown! | `FallDown` | 1.4 | ★★ | `34791fe2` | `falldown.md` |
| PapiJump | `net.sunflat.iphone.PapiJump` | 1.6.1.0.4 | ★★★ | `d387265f` | `papijump.md` |
| Dizzy Bee | `com.dizzybeegame.dizzybee` | 1.5 | ★★★ | `3b105947` | `dizzy-bee.md` |
| Dizzy Bee 2 | `com.dizzybeegame.dizzybee2` | 1.2 | ★★★ | `d60ed8c7` | `dizzy-bee-2.md` |
| DizzyBeeFree | `com.dizzybeegame.dizzybeefree` | 1.2 | ★★★ | `d60ed8c7` | `dizzy-bee-free.md` |
| Labyrinth | `se.codify.labyrinth` | 1.2 | ★★ | `6dd4671e` | `labyrinth.md` |
| Cut the Rope | `com.chillingo.cuttherope` | 1.6 | ★★★ | `4bb73ce7` | `cut-the-rope.md` |

The notes are in `dev-docs/app-notes/`, and each carries the evidence, the
route, and what was and was not measured.

## Boundaries that are lost

Five of the apps above went from unrated to three stars in one session, so they
passed two stars on the way and that boundary was never filed either. It cannot
be reconstructed: `compatibility/README.md` is explicit that a boundary passed
without a report cannot be filled in later from a note or from a rerun on a
newer revision.

That is written down rather than quietly dropped, and it is the cost of the
outage rather than of the work.
