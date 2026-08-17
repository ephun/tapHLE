# Fantastic Mr. Fox compatibility work note

Last updated: 2026-07-18.

## Identity and source

- Work branch: `compat/fantastic-fox`.
- Archive item: <https://archive.org/details/ios-ipa-com.fox.fmf>.
- Tested file: `Fantastic Mr. Fox - US (v1.0) [Decrypted].ipa`.
- SHA-256: `468d3418013cb17f0eebc00a0e1b3ec1396dd6e0184f9e3a120211a499b9f090`.
- MD5: `91c18541a7747f0ee0c526ca894d8b60`.
- SHA-1: `12105124c847a84e3836e7e8c82fac2ce10b5b77`.
- Bundle: `com.fox.fmf`, version `1.0`, minimum OS `2.2.1`.

The verified IPA is kept locally in ignored `tapHLE_apps` for picker and direct
playtesting. It is not part of Git.

## Current checkpoint

Exact release commit `25dcc7b692360f66931471b191c04ed8600cbbaa`
reaches live Level 1 gameplay on Windows. The loading screen, main menu,
18-level selector, Level 1 story screen, and gameplay scene all render. The
gameplay capture shows Mr. Fox, hazards, the HUD, health, and an advancing
timer. The process remained alive until the harness stopped only that process.

The compatibility rating is **3/5 In game**. Menu input works, but gameplay
controls and a complete level have not been validated yet.

## Fixes that moved the frontier

- Guest `methodForSelector:` can return a guest method implementation.
- `NSOperation`, `NSInvocationOperation`, and `NSOperationQueue` support the
  operation-based loading path used by the game's audio library.
- `NSXMLParser` repairs a mismatched closing tag while preserving later sibling
  events; the shipped `Sprites.xml` contains one misspelled closing tag.
- `UIApplication statusBarFrame` reports early UIKit portrait-coordinate
  geometry.
- Mutable and immutable sets implement real copy and mutable-copy behavior,
  which allows touch dispatch to continue after the main-menu Play tap.

## Known gaps

- A brief white bar flashes after window creation and before the loading screen.
  It is not part of the loading art. Determine whether it is the initially
  visible status-bar area or the window's default background before changing
  rendering behavior.
- Gameplay movement or tilt controls, pausing, death, restart, level completion,
  and later levels are untested.
- Saving and long-session stability are untested.
- Sound was heard in an earlier exploratory run, but audio coverage was not
  revalidated on the exact gameplay checkpoint.
- The game requests `HelveticaNeue-Bold`. The nearest bundled sans bold font is
  now used. Its focused test passes, and a provisional replay removed the 546
  repeated fallback warnings seen in the earlier short gameplay run. The
  database report remains tied to the earlier exact clean gameplay commit.

## Clickmap

`dev-docs/clickmaps/fantastic-mr-fox.json` replays launch to Level 1
gameplay. Run it with `dev-scripts/clickmap.ps1` before working out a route
by hand. The prose route below is the same thing with the reasoning attached.

## Reproduction route

Use the exact hash-verified IPA and a release build. The emulator client is
480x320 after rotation.

1. Click the window title bar and confirm the tapHLE window owns foreground
   focus.
2. Main menu **Play**: client coordinate `(240,247)`.
3. **Level 1**: client coordinate `(57,130)`. A 120 ms press was reliable.
4. Story screen **Play**: client coordinate `(240,282)`.
5. Wait for the Level 1 HUD and moving timer before capturing the frame.

The EAGL capture is 320x480 with a bottom-left origin. For the verified
landscape-left view, inspect it with `vflip,transpose=2`.
