# Snappers compatibility work note

- Branch and starting commit: `compat/snappers` from `b315e827`.
- Canonical artifact: `https://archive.org/details/ios-ipa-ru.emergingmobile.snappers`,
  `Snappers (v1.08) [Decrypted].ipa`; MD5
  `d831248dc1c358a8ad65c197ed2b43a9`, SHA-1
  `273938ce60108d5424b19cd04c90c59aa73b40c2`, SHA-256
  `69ade8efac1958a2259ff8cebc5e79e2953606409a1787307ca62bb4dafeafc6`.
- Embedded identity verified with `tapHLE --info`: display name `Snappers`,
  bundle `ru.emergingmobile.snappers`, version `1.08`, minimum OS `3.1`.
- Availability check (2026-07-24): Apple’s US bundle-ID lookup returned no
  current listing for this exact build. This is a project-scope availability
  fact, not a legal conclusion.
- Windows evidence on committed release build `169bf157` (2026-07-24): a
  fresh launch reached the menu, tutorial, Level Select, and Level 1. Tapping
  the Level 1 target reduced `Taps left` from 1 to 0 and displayed
  `Completed! Score: 50`; the process remained active for at least ten seconds
  after the tap.
- Reusable paths added during that run: missing Objective-C method signature
  queries return `nil`; `ExtAudioFile` exposes file frame length; mutable
  strings and arrays implement the collection mutations the game uses;
  `NSValue` supports non-retained object wrappers; `NSURL` resolves a relative
  string against a base URL; and run-loop dispatch discards an unused object
  argument for zero-argument selectors.
- Highest clean committed milestone: 3 stars on `169bf157`, pending database
  publication and merge to `trunk`.
- Next discriminator: submit the 3-star report, merge `169bf157` to `trunk`,
  and push both the integration branch and this continuation branch.

## 2026-07-27: 3 stars re-confirmed on tapHLE `87acd74a`

Re-driven end to end on a clean committed build after a session of broad UIKit
changes: menu Play `(160, 240)` -> level pack `(160, 190)` -> level grid
`(160, 250)` -> Level 1 `(50, 68)` -> tap the snapper `(160, 210)`.

The result matches the original `169bf157` evidence exactly: `Level: 1-1`,
`Taps left` goes 1 -> 0, and the screen shows **`Completed!` / `Score: 50`**.

### Click map, recorded because it was missing

The original note described the route in prose but gave no coordinates, which
cost a re-verification run rediscovering them. 320x480 client, ~30 s launch:

1. Title with the round Play button -> `(160, 240)`.
2. Level pack (photo carousel) -> `(160, 190)`.
3. -> `(160, 250)` reaches the 5x5 level grid; only Level 1 is unlocked.
4. Level 1 -> `(50, 68)`.
5. The snapper -> `(160, 210)` completes the level.

**A static screen is not a failure here.** Snappers is a puzzle game: an
unsolved board does not animate, so comparing two captures by hash — the check
that works for an action game — reports "static" for a perfectly healthy
Snappers. Drive it to a completed level instead.
