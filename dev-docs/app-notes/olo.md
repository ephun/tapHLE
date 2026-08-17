# OLO compatibility work note

- Branch: `compat/olo`.
- Artifact: `OLO (v1.23) [Decrypted].ipa` from the maintainer's local
  collection, 6,195,536 bytes, locally computed SHA-256
  `6fe7e974dc700647650150c9282b8742b713e006c60eb880db26145faf627509`.
  **Provenance is the local collection, not a verified Archive item**, and
  availability was not re-checked; both are needed before this is treated as a
  public archive-backed result.
- Embedded identity (`tapHLE --info`): display name `OLO`, bundle
  `com.Sennep.OLOgame`, version `1.23`, minimum OS `4.3`, iPhone and iPad.
- The 1.31 copy in the collection is the same bundle identifier at a later
  version and needs iOS 5.0.

## Highest milestone: 3-star (In game), tapHLE `8c12f99d`

A two-player round starts and stays live. The board draws with its pink and
mint zones, green pieces down one side and red down the other, and a piece in
play whose glow keeps animating: two screen captures twelve seconds apart differ
only in the region around it. The round is real rather than an attract screen —
tapping through to the pause menu gives *Resume* and *Menu*.

Route: `dev-docs/clickmaps/olo.json`. One tap at `(160, 430)` after boot.

## What it took

This app declares `opengles-2`, and it was the app the OpenGL ES 2.0 work was
measured on. Two general fixes, both on `trunk`:

1. `-[UINavigationController visibleViewController]`, which did not exist; the
   app asks for it during startup and stopped there.
2. The ES 2.0 present path did not flush before restoring the app's framebuffer
   binding, so every frame was drawn correctly and then discarded. The window
   was black while the app's own renderbuffer held the menu.

tapHLE also stopped claiming at launch that only ES 1.1 is supported, which had
been untrue for some time and is what made this app look unreachable.

## Known faults at this rating

- **The board is drawn a quarter turn out.** When the round starts the window
  becomes landscape and the content is rendered rotated inside it, so labels
  read bottom-to-top. Playable to look at, wrong to read.
- **The menu's labels are not where its buttons are.** A tap on `2 Player`
  reaches the app and is hit-tested — `ccTouchBegan:` is sent to `ButtonMenu`
  and `ButtonSubmenu` — and neither claims it. Something differs between the
  coordinate space the app lays its buttons out in and the one it draws them
  in; the same suspicion covers the rotation above.

## Next discriminator

Both faults above look like one cause: a disagreement about the screen's size
or orientation between layout, drawing and touch. Start by logging what the app
is told by `-[UIScreen bounds]`, `-[UIScreen applicationFrame]` and the EAGL
layer's size at startup, and compare with the 320x480 window it actually gets.
Fixing that would likely make the menu respond where it is drawn *and* land the
board the right way up, and it is worth doing before any per-app coordinate
workaround.
