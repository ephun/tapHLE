# Glass Tower 2 compatibility work note

- Branch: `compat/glass-tower-2`. Reusable fixes graduated to `trunk`.
- Canonical artifact: <https://archive.org/details/ios-ipa-collection>,
  `Glass Tower 2 1.7.ipa`; size 5,020,776 bytes. (An earlier revision of this
  note recorded 6,060,474 — that is Cops & Robbers' size, copied by mistake.)
- Hashes:
  - MD5: `33643ef687ff93edb5e3647224280a14`
  - SHA-1: `3d6080e91a1ef4cf260c4db8e81faa90b25aae3e`
  - SHA-256: `5383f5eab7a54f6984bbb0797e1b295952371a7981f3132d54699e2e5a9df421`
- Embedded identity (`tapHLE --info`): display name `GlassTower2`, bundle
  `com.idevua.glasstower2`, version `1.7`, minimum OS `3.0`, iPhone.
- Options: none. Window is 320x480 portrait.
- tapHLEdb: App 11, version 11. Report 19 (2026-07-26, tapHLE `a490a841`,
  ★★★☆☆) supersedes the earlier 2-star report.

## Highest milestone: 3-star (In game), tapHLE `a490a841`

Reproduced from a clean committed release build (window title
`GlassTower2 (tapHLE a490a841)`, no `-dirty`) against the hash-verified bytes.

Main menu -> GLASS TOWER GAME -> NEW GAME -> SELECT LEVEL PACK -> Free Level
Pack -> level 1. In level 1 the tower renders over the score/lives HUD, tapping
a block removes it, the physics runs and the remaining blocks settle, and the
HUD updates: Score 0 -> 10, Lives 10 -> 9 with a red `-10` popup after removing
a red block.

### Click map

No launch options; window 320x480 portrait; wait ~12 s for the title menu.

1. Title menu -> `(160, 128)` GLASS TOWER GAME -> game submenu.
2. Submenu -> `(160, 150)` NEW GAME -> SELECT LEVEL PACK.
3. Level pack -> `(160, 128)` Free Level Pack -> level 1 gameplay.
4. Level 1 -> `(122, 320)`, `(197, 320)` blocks -> removed, HUD updates.

## What was wrong

The previous 2-star frontier was "menu touch/click input is unhandled or
blocked from starting a level". That had two independent causes, both general
emulator gaps and both now fixed on `trunk`:

1. `viewDidLoad` was never sent to a view controller unarchived from a nib, so
   the app's touch coordinate mapping was never initialised. Diagnosed in
   detail on Glass Tower HD — see `glass-tower-hd.md`. Fixed in `295dbcc4`.
   With that alone, the first menu tap started reaching the app.
2. Three documented mutable-collection methods were missing, and each one
   aborted the emulator with a "does not respond to selector" panic at the next
   menu step: `-[NSMutableArray initWithObjects:]` on the first tap,
   `-[NSMutableArray setArray:]` on NEW GAME, and
   `-[NSMutableDictionary setDictionary:]` on choosing a level pack. Fixed in
   `0cd40817`.

The array case is worth remembering: `_tapHLE_NSMutableArray` descends from
`NSMutableArray`, not from `_tapHLE_NSArray`, so it does **not** inherit the
immutable class's implementations. Anything NSArray declares has to be provided
on both concrete classes.

## Earlier fix retained

- Handled `UIDeviceOrientationPortrait` in `src/environment.rs`'s startup
  interface-orientation matcher.

## Known limitations at this rating

- Only the Free Level Pack, level 1, and only two blocks removed. Whether a
  level can be completed or the level advances is unknown.
- CONTINUE, USER LEVELS, HELP, HI-SCORES, SHOP and OPTIONS were never opened.
  The three paid level packs are locked and were not touched.
- Audio was not measured.
- `[UIActivityIndicatorView stopAnimating]` is an unimplemented TODO that fires
  during level loading. It did not prevent the level from loading.

## Checks run

- `cargo fmt --all -- --check`
- `cargo test --workspace --lib` (91 passed)
- `cargo build --release`

## Next discriminator

Complete level 1 and see whether the game advances to level 2; that is the
boundary between this rating and a 4-star human test.

## 2026-07-27: 3 stars re-confirmed, by OS screenshot

Driven to gameplay on `73a43594`: title `(160, 128)`, then `(160, 150)`, then
`(122, 320)` reaches **SELECT LEVEL PACK**, and `(160, 165)` picks the free pack
and starts **Level 1** — the block tower renders with `SCORE: 0` and `LIVES: 10`.

Verified with an **OS-level `PrintWindow` screenshot**, not tapHLE's frame
capture: 1032 distinct sampled colours. See the playbook's "a frame capture is
not necessarily the screen" — a capture can show a perfect image for an app that
is presenting nothing, which is how The Jim & Frank Mysteries HD was rated two
stars in error.

The click map previously stopped at the level-pack selector, which is a menu and
not gameplay; the missing fourth tap is recorded above.
