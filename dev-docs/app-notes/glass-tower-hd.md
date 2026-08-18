# Glass Tower HD compatibility work note

- Branch: `compat/glass-tower-hd`. Reusable fix graduated to `trunk`.
- Canonical artifact: <https://archive.org/details/iPadGames2010>,
  `Glass.Tower.HD-v1.0.ipa`; size 10,727,706 bytes.
- Hashes:
  - MD5: `cafb203ca6720949f4dbbb4e2010bed3`
  - SHA-1: `f3acfe7922dc42e9e5507a107ab1f6d0d37d3c63`
  - SHA-256: `80589a0a7d48117641a6badad06300907d1349c42396561ce46bab9954cfe879`
- Embedded identity (`tapHLE --info`): display name `Glass Tower`, bundle
  `ru.idigger.Glass-Tower`, version `1.0`, minimum OS `3.2`, iPad.
- Options: `--force-composition`.
- tapHLEdb: App 10, version 10. Report 18 (2026-07-26, tapHLE `6eab1c98`,
  ★★★☆☆) supersedes the earlier report 16.

## Highest milestone: 3-star (In game), tapHLE `6eab1c98`

Reproduced from a clean committed release build (window title
`Glass Tower (tapHLE 6eab1c98)`, no `-dirty`) against the hash-verified bytes,
in a normal visible 768x1024 window.

Main menu -> CHOOSE MODE -> BUILT-IN LEVELS -> level 1. In level 1, tapping a
blue block removes it, the physics runs (blocks above topple and fall), a `+10`
popup with particles appears, and Score/Hi-Score rise 0 -> 10 -> 40 across
three taps. Level, Lives (15) and the close button all render.

### Click map

The route is recorded as `dev-docs/clickmaps/glass-tower-hd.json` and was replayed end to end
on `e4e0d528` on 2026-08-17. Replay the map rather than retyping the steps
below; the prose is kept for the reasoning behind the coordinates, and the map
is what should be kept correct.


Launch `--force-composition`; window 768x1024 portrait; wait ~10 s for the
title menu before the first tap.

1. Title menu -> `(384, 375)` NEW GAME -> CHOOSE MODE.
2. CHOOSE MODE -> `(384, 370)` BUILT-IN LEVELS -> level 1 gameplay.
3. Level 1 -> `(340, 730)`, `(480, 795)`, `(271, 930)` blue blocks -> each is
   removed and scores +10.

## What was wrong: viewDidLoad was never sent

The earlier note recorded 3 stars, but on re-test the menu rendered and
animated at 30 fps while **no tap did anything anywhere on screen**. The
diagnosis ladder was:

1. Touch reached the app: `ui_touch` hit-testing found the app's own
   `MyEAGLView` (full-screen), and `touchesBegan:`/`touchesEnded:` were both
   delivered. No "couldn't find a view" warning.
2. `UIResponder`'s "(probably unhandled)" default never logged, so the app's
   own handler was running.
3. Static inspection: `-[MyEAGLView touchesBegan:withEvent:]` is a thin
   forwarder to `[[[UIApplication sharedApplication] delegate] touchesBegan:...]`;
   `-[ipadEngineAppDelegate touchesBegan:withEvent:]` switches on its `appMode`
   ivar (1/2/3 -> `myMenu`/`myGame`/`myLevelEditor`) and drops the touch for any
   other value. A selector trace confirmed the whole chain ran through to
   `-[Menu touchesBegan:withEvent:]`, so `appMode` was 1 and the Menu object
   was live.
4. `-[Menu stepMenu:]` and `-[Menu drawMenu]` ran ~29x/second, so the update
   loop was not stalled either. (An early CPU-time reading of ~0 s suggested a
   stall; it was misleading. Counting traced ticks is the reliable liveness
   check, not `TotalProcessorTime`.)
5. `-[Menu touchesBegan:withEvent:]` maps the touch through a two-float global
   `_koff` as `(coord - koff[0]) * koff[1]`. `_koff` is written only by
   `-[viewControl setKoff]`, which is called from `viewDidLoad` and from
   `didRotateFromInterfaceOrientation:`. A selector trace showed **`setKoff`
   and `viewDidLoad` were never called at all**, so `_koff` stayed zeroed and
   every tap collapsed to the origin.

Root cause, and it is general rather than app-specific: tapHLE sent
`viewDidLoad` only from the programmatic route (`-view` finds nil ->
`-loadView` -> `viewDidLoad`). A controller unarchived from a nib gets its view
from `-[UIViewController initWithCoder:]` calling `-setView:` directly, so
`-loadView` never runs and `viewDidLoad` was never sent. Fixed in `295dbcc4`
(merged as `6eab1c98`): a single helper sends it at most once per load, from
`-view` and from the nib loader after outlets are connected and
`awakeFromNib` has been sent.

## Regression check for that fix

Nine apps already at 3 stars were relaunched on `295dbcc4` and all still
reached their expected screens: Cops & Robbers (also replayed through its full
click map into gameplay), Baby Monkey, Fantastic Mr. Fox, Flight Control HD,
Percy Jackson, Ricky, SPY mouse HD, Snappers, Warlords HD.

## Known limitations at this rating

- Only level 1 of BUILT-IN LEVELS was played, and only three blocks were
  removed. Whether a level can be *completed*, whether the level advances, and
  what losing a life does are all unknown.
- USER LEVELS, OPTIONS, CONTINUE and the level editor (`LevelEditor`, reachable
  at `appMode` 3) were never opened. The nib contains an "Enter filename"
  `UITextField` for user levels that tapHLE only partially supports
  (`setClearButtonMode:`, `setBorderStyle:`, `setKeyboardType:` are TODOs).
- Audio was not measured at all for this app.
- Every frame logs `There is no fullscreen layer, presenting renderbuffer ... by
  copying to RAM (slow path)` with a ~3 ms `glReadPixels(0,0,768,1024)`, and
  `Too much slop accumulated, skipping an interval`. It runs at ~30 fps rather
  than 60. This is a `--force-composition` cost, not a correctness problem.
- `UIProxyObject` for `IBFirstResponder` is left unreplaced at nib load.

## Checks run

- `cargo fmt --all -- --check`
- `cargo test --workspace --lib` (91 passed)
- `cargo build --release`

## Next discriminator

Play level 1 to completion to see whether the level advances and what the
inter-level screen looks like; that is the boundary between this rating and a
4-star human test. After that, check whether `--force-composition` is still
required now that the app is interactive, since dropping it would remove the
per-frame readback.
