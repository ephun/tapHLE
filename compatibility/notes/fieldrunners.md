# Fieldrunners compatibility work note

- Branch: `compat/fieldrunners`. The fix that moved it graduated to `trunk` as
  `fix/fs-rename-directory`.
- Identity of the build this note is about, read from `tapHLE --info` and not
  from the filename: display name `Fieldrunners`, bundle
  `com.subatomicstudios.Fieldrunners`, version `1.5.3`, canonical
  `Fieldrunners.app`, minimum OS `3.0`, no required capabilities, device family
  iPhone. SHA-256 `eef3b0d0…b00105`.
- Local copies from the maintainer's collection, not Archive-backed.
- Route: `compatibility/clickmaps/fieldrunners.json`. No launch options; the app
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

## 2026-08-19, later: it plays, and why that is still two stars

On `d387265f`, from a fresh profile, `compatibility/clickmaps/fieldrunners.json`
replays the whole way: main menu, Select Your Map, the Grasslands tile,
Customize with its difficulty and modes, Start, the tutorial overlay, and then
live play. Round 1 becomes Round 3 and the lives count falls from 20 to 13
while runners cross, so the loop is running rather than a held frame.

Five general fixes were needed between the menu and that, in this order, each
one the app's next stop:

- `NSScanner` copied the whole rest of the string on every scan
  (`fix/scanner-scans-without-copying`).
- `appendString:` rebuilt the whole string on every append
  (`fix/mutable-string-appends-in-place`). The two together were most of an
  11 GB level load.
- `scanUpToString:` reported success when it had scanned nothing, so the
  parser's loop never ended — 12.2 million scans at the end of a
  seven-character string (`fix/scanner-stops-at-the-end`).
- `NSScanner` had no `scanFloat:` (`feat/scanner-reads-a-decimal-number`).
- `CFBinaryHeap` did not exist at all, and the level's route-finding is built on
  it (`feat/core-foundation-binary-heap`), then
  `getBytes:maxLength:usedLength:encoding:options:range:remainingRange:` was
  missing for the tap that leaves the tutorial overlay
  (`feat/foundation-string-get-bytes`).

**It is not three stars, because the playing field does not draw.** The HUD, the
runners, the overlays and the tower prices all appear, over white. Three stars
covers rendering, and a game whose field is missing is not there yet.

The next discriminator: the log carries **222** occurrences of
`No EAGLContext for thread 1! Ignoring OpenGL ES call`, so the app makes GL
calls from a second thread that has no context of its own, and tapHLE drops
them. Whether those calls are what draws the field is the thing to establish —
not assumed — and the way to establish it is an internal EAGL capture rather
than a window capture. If it is, the work is real: a second context sharing
textures with the first, or those calls serialised onto the thread that has one.

**A second problem, recorded because it will waste someone's time otherwise:**
once the app has a save, the route stops working. With
`Documents/default.sav` present, the Grasslands tap does nothing at all —
three runs, no change to the frame — and with the sandbox deleted it works
every time. The clickmap marks every step past the menu as needing a fresh
profile. A game that cannot be replayed after being played once is its own
compatibility problem, and nothing is known yet about which of the two paths is
at fault.

## 2026-08-19, later still: 1.2.3 reaches its menu too

`1.2.3` — display name `Fieldrunners`, bundle `com.subatomicstudios.Fieldrunners`,
canonical `Fieldrunners.app`, minimum OS `2.0`, iPhone, SHA-256 `3e83ae6a…3a07dd`
— stopped at `NSScanner`'s missing `scanFloat:` before today and now boots to
its main menu: the same title art with PLAY, RESUME, SCORES and HELP, and no
SOCIAL, which that build does not have. **1★ → 2★ on `d387265f`.**

The route is the first two steps of `compatibility/clickmaps/fieldrunners.json`; the
map itself stays 1.5.3's, since a map belongs to an app rather than to a build,
and the steps past the menu have not been checked against this one.

**Its report is blocked by the same outage** — `https://taphle.ephun.net/`
still returns HTTP 522 — so this boundary joins 1.5.3's as an open threshold.
Submit whatever rating each build holds when the endpoint returns; do not
back-date either.

## What the survey says about the whole family, after today

Re-surveyed all 124 files at `d387265f` and compared against `63119bcf`:
**zero regressions**, and the two builds that moved are both this app's —
`1.2.3` off `scanFloat:` and `1.5.3` off the directory rename. That the other
fixes do not show is expected rather than disappointing: the survey presses no
buttons, and everything after the scanner work only matters once something has
been tapped.

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
