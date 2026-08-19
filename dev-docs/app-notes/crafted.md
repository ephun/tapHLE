# Crafted compatibility work note

- Branch: `compat/crafted`. Reusable fixes graduated to `trunk`.
- Identity, read from `tapHLE --info` and not from the filename: display name
  `Crafted`, bundle `com.Pickl.Crafted`, version `1.0.1`, minimum OS `4.0`,
  device family iPhone, requires `accelerometer` and `opengles-1`.
- Local copy from the maintainer's collection, not Archive-backed.
- tapHLEdb: app 33, version 34, report 65 (2026-08-05, tapHLE `10075dc6`,
  ★★★☆☆).

## The 1★ → 2★ boundary was never reported, and cannot be

Recorded because the guide requires the omission to be written down rather than
papered over, and because the cause is reusable.

This app crossed 1★ → 2★ (a stable main menu) partway through the same session
that took it to 3★. It was never filed, and it cannot be filed now: a report
asserts that an artifact was run and rated at a specific revision, and
reconstructing that afterwards from an app note would be asserting something
nobody did.

**The cause was chaining fixes across the boundary on a dirty worktree.** Two
fixes were applied, the app was rerun, and it reached a stable menu — but the
build was `-dirty`, and a dirty-worktree result is barred from the database. The
obvious next move was the next blocker, so the tree stayed dirty through three
more fixes, and by the time there was a clean revision to cite, the app was
already at 3★ and the 2★ moment had passed.

That is not a slip; it is what happens by default when an agent iterates. The
moment a fix works is exactly the moment the tree is dirty. The remedy is in
`AGENTS.md`: stop at a boundary, commit, rebuild clean, re-verify, publish, and
only then start the next blocker.

## 2026-08-19: the saved-world panic is fixed, and the next blocker is text

`Fs::size()` now answers for a directory, so the Singleplayer tap on a profile
that already has a saved world no longer panics tapHLE. Fixed on `trunk` in
`fix/fs-directory-size` (commit `82595e15`); the change is general emulator
behaviour, not this app's.

Verified on `82595e15`, a clean build, in two runs against the hash-verified
local copy (`940c9074…`):

- `dev-docs/clickmaps/crafted.json` replayed end to end on a fresh profile —
  world generated, player on terrain, hearts and hotbar drawn, still live half a
  minute later.
- Relaunched with `Documents/saves/New World/` in place. The Singleplayer tap
  reached the attributes call and got past it: no `fs.rs` panic, and neither of
  the new "no size available" / "no modification time available" warnings was
  logged, so both attributes were answered.

**The second launch still does not reach the world list**, on a different and
later fault:

```text
thread 'main' panicked at src\objc\messages.rs:432:13:
Object … (class "_tapHLE_NSMutableString") does not respond to selector
"replaceCharactersInRange:withString:"!
```

That is the one primitive `NSMutableString` subclasses must implement, and
`ns_string.rs` says so in a comment while not implementing it — everything else
(`appendString:`, `deleteCharactersInRange:`, `insertString:atIndex:`,
`replaceOccurrencesOfString:…`) is built on top of it and exists. So this is a
Foundation gap that any app editing a mutable string in place will hit, and it
gets its own branch rather than riding along here.

So the rating is unchanged: three stars on a fresh profile, and a saved world
still cannot be reopened — one blocker further along than before.

## 2026-08-17: the route is a clickmap, and a saved world breaks the app

The route is recorded as `dev-docs/clickmaps/crafted.json` and was replayed end
to end on `e4e0d528`: main menu -> Singleplayer `(240, 154)` -> Create New World
`(366, 247)` -> the creation form's own Create New World `(240, 247)` -> a
generated world, still live and animating thirty seconds later. Replay the map
rather than retyping the steps.

**Recording it turned up a tapHLE bug that the original run could not have
seen.** The 2026-08-05 session created its world and stopped. Replaying the
route a second time does not get past the first tap: Singleplayer panics tapHLE
with

```text
thread 'main' panicked at src/fs.rs:875:18:
not implemented
```

The Select World screen asks `NSFileManager` for the saved world's attributes,
that asks `Fs::size()` for the size of a **directory**, and `size()` handles
only the two file cases and falls through to `unimplemented!()` for everything
else. Reproduced three times, and it goes away exactly when
`tapHLE_sandbox/com.Pickl.Crafted/Documents/saves/<world>` is deleted.

So this app is three stars on a fresh profile and cannot reach its own world
list on the second launch — which is worse than the rating suggests, because
saving a world is the normal thing a player does. The fix is general and
belongs to `Fs::size()`, not to this app: a directory has a size on every host
tapHLE runs on, and any app that stats one hits this. It was fixed on `trunk`
in `fix/fs-directory-size` on 2026-08-19; the section above records what the
second launch does now.

## 2026-08-05: three stars on `10075dc6`

Reaches a gameplay loop that starts and persists. Verified with OS-level
`PrintWindow` screenshots on a clean worktree, driven by scripted input:

- Main menu → Singleplayer → Select World → Create New World → a generated
  world with the player, terrain, health bar, hotbar and touch controls.
- The world is written to disk: `Documents/saves/New World/{0,main}`.
- It responds to input in-game — pressing the movement control changes the
  scene — so the loop is live rather than a frozen frame.
- Zero panics across the run; the scene is unchanged 15 s after generation.

**It needs `--landscape-left`.** The app declares no orientation in its
`Info.plist` at all, and tapHLE only consults
`shouldAutorotateToInterfaceOrientation:` when the window is *already*
non-portrait, so nothing ever asks the app which way up it wants to be. Launched
portrait, it draws its 480-wide layout into a 320-wide screen and the
"CRAFTED PIXELS" splash is visibly cut off. This is general, not app-specific:
1167 of 1501 apps in one collection declare no `UIInterfaceOrientation`, and 366
of those ship `Default-Landscape` art. Fixing it is separate work.

## What was in the way, in the order it was hit

Each one only became visible once the previous was cleared, which is why this
took five rounds rather than one survey.

1. `alcGetIntegerv` was a `todo!()` stub. Died about five seconds in, before
   drawing anything. → `fix/openal-alc-get-integerv`.
2. `ExtAudioFileSeek` was unimplemented. → `fix/ext-audio-file-seek`.
   With these two the app reaches a stable main menu (two stars).
3. `-[NSFileManager fileExistsAtPath:isDirectory:]` computed `isDirectory` as
   `!is_file(path)`, which is **true for a path that does not exist**. The app
   checks three folders at startup, was told all three were already there,
   created none, and failed at world creation with a nonexistent parent
   directory for `Documents/saves/New World` — three screens later.
   → `fix/file-exists-is-directory`. This was the real blocker.
4. Four `NSError**` sites aborted instead of reporting: three `todo!()`s and an
   `assert!(error.is_null())` that fired even on success.
   → `fix/nserror-out-parameters`.
5. `Fs::modified` hit `unimplemented!()` for directories. Only appears on the
   *second* launch, when there is a saved world to sort by date.
   → `fix/directory-modification-time`.

## Driving it without a person at the keyboard

Worth recording because half a session went into it. Synthetic clicks via
`SetCursorPos` + `mouse_event` do **not** work: Windows refuses
`SetForegroundWindow` to a background process, so the click lands on whatever
window actually has focus and the guest sees no touch at all. Posting
`WM_LBUTTONDOWN`/`WM_LBUTTONUP` straight to the window handle does reach SDL,
but only while the window happens to hold focus, so it succeeds in some runs and
silently does nothing in others — which desynchronises a fixed-timing click
script and looks exactly like a broken app.

What works: move the real cursor, and click each target **twice**, because the
first press is consumed activating an unfocused window. Then verify each step
actually changed the screen and retry if it did not, rather than trusting a
delay. A tapHLE window is visible and interactable on the maintainer's desktop
from Claude Code — the background-desktop problem in `AGENTS.md` is AGY-specific
and does not apply here.

## Next

- Nothing has been tried beyond generating a world and moving: no mining,
  placing, saving-and-resuming, or mob interaction.
- Resuming an existing world exercises a different path — it is what turned up
  the directory-modification-time crash — and has not been re-tested since.
- Still missing statically, from `dev-scripts/demand.py app`: 29 symbols,
  mostly OpenAL capture and `UIImage{PNG,JPEG}Representation`. None blocked
  gameplay.
