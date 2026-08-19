# Fieldrunners compatibility work note

- Branch: `compat/fieldrunners`. The fix that moved it graduated to `trunk` as
  `fix/fs-rename-directory`.
- Identity of the build this note is about, read from `tapHLE --info` and not
  from the filename: display name `Fieldrunners`, bundle
  `com.subatomicstudios.Fieldrunners`, version `1.5.3`, canonical
  `Fieldrunners.app`, minimum OS `3.0`, no required capabilities, device family
  iPhone. SHA-256 `eef3b0d0…b00105`.
- Local copies from the maintainer's collection, not Archive-backed.
- Route: `dev-docs/clickmaps/fieldrunners.json`. No launch options; the app
  comes up landscape by itself.

## 2026-08-19: 1★ → 2★ on `e643eb3e`, and the report is blocked

The app reaches its main menu — title art with PLAY, RESUME, SOCIAL, SCORES and
HELP down the right, drawn the right way up and the right way round — and stays
there. Verified by replaying the clickmap on a clean build of `e643eb3e`, twice,
with the app still ticking its drawables at the end rather than sitting on a
last frame.

**What was in the way:** it moves a directory during start-up, and
`Fs::rename()` implemented only the file case, so tapHLE aborted with
`not implemented` before the app drew anything. Fixed generally in
`fix/fs-rename-directory`; nothing in that fix is specific to this app.

**The report for this boundary could not be submitted.**
`https://taphle.ephun.net/` returns **HTTP 522** — Cloudflare cannot reach the
origin — at the root as well as at `/compatibility/api/apps`, on four attempts
on 2026-08-19. The same outage blocked the Doodle Jump boundaries on
2026-08-18, so it has been down for at least a day. Threshold publication for
this app stays **open**: when the endpoint returns, submit the rating the app
holds at that moment on the revision tested at that moment, and do not
back-date it to `e643eb3e`.

## Where the other builds in the collection stop

Surveyed at `63119bcf`; the family is eight files and four different frontiers,
so they are not one problem:

- `1.2.3` — `NSScanner` has no `scanFloat:`. A missing method, and the
  cheapest of these to close.
- `1.3.0`, `1.3.2` — guest null-page read at `0x0`, PCs `0x2bfb4` and
  `0x2c198`. Two builds of the same engine, so probably one cause.
- `1.5.3` — the directory rename, fixed; now at two stars.
- `Fieldrunners for iPad 1.0.1` — an assertion in
  `core_graphics/cg_bitmap_context.rs` (`left == right`), which is tapHLE's own
  and worth reading before anything else here.
- `Fieldrunners 2 1.0`, `1.1` — survive 30 s of unattended start-up, so they
  are the closest to a rating in the family and need a route rather than a fix.
- `Fieldrunners 2 1.4` — guest null-page read at `0x18046a`.

## Next step for 1.5.3

Press PLAY and find out whether a level starts, which is the 2★ → 3★ question.
The menu buttons are down the right-hand side; read their coordinates off a
capture and subtract the window border and title bar before recording them as
client coordinates, or the taps land in the gaps between buttons.

The `Call to faked class "OpenFeint"` lines that fill the log every frame are
the absent social SDK, not a fault, and SOCIAL is the button that leads to it.
