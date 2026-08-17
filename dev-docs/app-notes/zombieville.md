# Zombieville USA compatibility work note

- Branch: `compat/zombieville`.
- Artifact: `Zombieville USA (v1.0) [Decrypted].ipa` from the maintainer's local
  collection. **Provenance is the local collection, not a verified Archive
  item**, and availability was not re-checked.
- Embedded identity (`tapHLE --info`): display name `Zombieville`, bundle
  `com.KelliNoda.Zombieville`, version `1.0`, minimum OS `2.0`, iPhone.

## Highest milestone: 3-star (In game), tapHLE `0cb76998`

A full round plays, with no launch options. Touch the title, NEW GAME, and the
level starts: the character with his gun, zombies walking in from the right,
ammo `x100`, a health bar, `$0`, and a walk arrow in each bottom corner. Tapping
the right arrow scrolls the street. Left to it, the zombies kill him and the
`YOU DIED` screen appears — the round ending as it should.

Route: `dev-docs/clickmaps/zombieville.json`.

## Nothing app-specific was needed

It ran on `trunk` as it stood. The only oddity in the log is a run of
`mmap could not allocate at hint` warnings, which are advisory: tapHLE places
the mapping somewhere else and the app carries on.

## What stopped it being found sooner

Nothing about the app. Earlier attempts to drive it — and three other apps —
concluded that "buttons are drawn but do not respond". That was wrong, and the
cause was in the testing rather than the emulator: this display scales at 175%,
tapHLE's window is DPI-unaware, and converting client coordinates through
`ClientToScreen` from a DPI-aware harness puts the cursor well away from the
button. Measuring the target from a screen capture and clicking at that offset
from the window's origin makes every one of those buttons work.
