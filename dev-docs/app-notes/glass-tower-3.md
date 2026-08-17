# Glass Tower 3 compatibility work note

- Branch: `compat/glass-tower-3`. Reusable fixes graduated to `trunk`.
- Canonical artifact: <https://archive.org/details/ios-ipa-collection>,
  `Glass Tower 3 2.0.1.ipa`; size 24,515,360 bytes.
- Hashes:
  - MD5: `56bafeb6bf4fad62e5115eea5165e9cf`
  - SHA-1: `0cdb51c48abf825078851d3fa33952bd666b0410`
  - SHA-256: `123514a494fa81448264b56f128b8dd18eaf2e61594cf73a966b484c9a99e645`
- Embedded identity (`tapHLE --info`): display name `GlassTower3`, bundle
  `com.idevua.glasstower3`, version `2.0.1`, minimum OS `4.0`, iPhone + iPad.
- Options: none. Window is 320x480 portrait.
- tapHLEdb: App 12, version 12, report 20 (2026-07-26, tapHLE `1db8dcea`,
  ★★★☆☆).

## Highest milestone: 3-star (In game), tapHLE `1db8dcea`

Reproduced from a clean committed release build (window title
`GlassTower3 (tapHLE 1db8dcea)`, no `-dirty`) against the hash-verified bytes.

Main menu -> SELECT LEVEL (a 20-tile grid with a total-score and stars header)
-> level 1. In level 1 the tower renders with the Lives/Score HUD, tapping a
block removes it, the physics runs, score popups (`+20`, `-20`) and shatter
particles appear, and later levels' furniture — chains, magnets, hazard-striped
blocks — draws correctly. The rising-bonus tutorial overlay appears and the
game keeps running behind it.

### Click map

No launch options; window 320x480 portrait; wait ~18 s for the title menu.

1. Title menu -> `(160, 130)` PLAY -> SELECT LEVEL.
2. SELECT LEVEL -> `(58, 118)` level tile 1 -> level 1 gameplay.
3. Level 1 -> `(118, 155)`, `(110, 240)`, `(160, 300)` blocks -> removed, with
   physics, particles and score changes.

Levels are randomly generated, so the exact tower differs per run; tap any
block rather than expecting a fixed layout.

## What was wrong

This is the first app in this series needing the **iOS 4** API surface rather
than iPhone OS 3, so it uncovered a long chain of gaps. Every one was general;
none needed an app-specific workaround. In the order they were hit:

1. `-[CALayer setMasksToBounds:]`, `setBorderWidth:`, `setBorderColor:` — all
   absent. Stored and read back; the compositor still neither clips nor
   strokes, which was already a TODO there. (`b2023dfa`)
2. `-[UILabel setAdjustsFontSizeToFitWidth:]` was `assert!(!adjusts)`, so any
   app setting it aborted. Implemented as real shrink-to-fit, with
   `-[UIFont fontWithSize:]` added to support it. (`b2023dfa`)
3. `ADBannerView` — the iAd framework was missing entirely. Added as a banner
   that never fills and reports that through the delegate. (`b2023dfa`)
4. `+[UIView animateWithDuration:animations:completion:]` — the iOS 4
   block-based animation API was missing. (`b2023dfa`)
5. `NSClassFromString(@"GKLeaderboardViewController")` panicked instead of
   returning nil. (`69d3915c`)
6. The completion block above was **retained rather than copied**. See below.
   (`72f36741`)
7. `-[NSArray pathsMatchingExtensions:]` and `srandomdev()` — absent.
   (`72f36741`)
8. `glEnableClientState(GL_POINT_SIZE_ARRAY_OES)` asserted instead of
   reporting `GL_INVALID_ENUM`. (`3b6a663c`)
9. `glPointSizePointerOES` was not exported at all. (`dc1f8a03`)

### The one worth remembering: stack blocks must be copied

Item 6 was a bug introduced by item 4, and it is the trap the debugging
playbook warns about. Clang passes `+animateWithDuration:...:completion:` a
**stack** block literal, valid only while that call is on the stack. The
completion object outlives it by design — the block runs when the animation
stops, from the run loop. Retaining it left the block's invoke pointer aimed at
a dead frame, and the failure appeared far away as:

```text
Attempted null-page access at 0x0 (0x4 bytes)   PC: 0x00000004  LR: 0x3000a000
```

with a stack trace that named only a host function. Nothing pointed at the
animation code. What identified it was a `TAPHLE_TRACE_SELECTORS=all` run: the
last two traced messages before the fault were
`_tapHLE_UIView_BlockCompletion tapHLE_animationDidStop:finished:context:` and
`NSNumber boolValue`, i.e. the fault was on the very next thing that method
did — invoke the block. The Blocks ABI requires `-copy`, which promotes the
literal to the heap.

Note this also means `LR` in the `0x3000a...` range was a red herring: it looked
like libgcc's unwinder, which suggested a missing `___objc_personality_v0` (that
symbol *is* genuinely unresolved at startup). It was not the cause. Do not chase
that warning.

### Diagnostics that paid for themselves

Item 8's fix logged the offending enum once instead of asserting, and the very
next run printed `glEnableClientState(0x8b9c)` — confirming the
`GL_POINT_SIZE_ARRAY_OES` guess already sitting in a TODO next to the `ARRAYS`
table, and leading straight to item 9.

## Known limitations at this rating

- Only level 1 was played, for a handful of blocks. Whether a level can be
  *completed*, whether the level advances, and what losing all three lives does
  are unknown.
- Point sprites are drawn at a uniform `glPointSize` because
  `GL_POINT_SIZE_ARRAY_OES` is not modelled, so particle sizes are wrong. It is
  not visually obvious at this zoom but it is a real inaccuracy.
- OPTIONS, HELP, USER LEVELS, the shop (`$`) and Game Center were never opened.
  StoreKit classes are referenced, so a purchase path may appear there.
- `___objc_personality_v0` is an unresolved non-lazy symbol at startup. Nothing
  has been observed to depend on it, but an app that actually throws an
  Objective-C exception would presumably fail in the unwinder.
- Audio was not measured.
- `CALayer` masksToBounds and borders are stored but not honoured when
  compositing.

## Checks run

- `cargo fmt --all -- --check`
- `cargo test --workspace --lib` (91 passed)
- `cargo build --release`
- Regression sweep on Windows over Baby Monkey, Fantastic Mr. Fox, Flight
  Control HD, Percy Jackson, Ricky, SPY mouse HD, Snappers and Warlords HD: all
  still reach their expected screens, five with byte-identical frames.

## Next discriminator

Complete level 1 and see whether the game advances to level 2, and what the
inter-level screen shows. That is the boundary between this rating and a 4-star
human test.

## 2026-07-27: 3 stars re-confirmed, by OS screenshot

Driven to gameplay on `73a43594` with the taps already recorded here —
`(160, 130)`, `(58, 118)`, `(118, 155)`, `(110, 240)`. The block tower renders
with falling debris, a `2UP` bonus indicator and a live **"TAP RISING BONUS TO
CATCH! / BE QUICK!"** prompt.

Verified with an **OS-level `PrintWindow` screenshot** rather than tapHLE's frame
capture: 996 distinct sampled colours. See the playbook's "a frame capture is not
necessarily the screen".
