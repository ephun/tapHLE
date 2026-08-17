# Mr. AahH!! compatibility work note

- Branch: `compat/mr-aahh`. Reusable fix graduated to `trunk`.
- Canonical artifact: <https://archive.org/download/ios3-6-ipas/> (identifier
  `ios3-6-ipas`), file
  `Mr.AahH!!-(com.ponos.mraahh)-1.2.0-(iOS_3.0)-646e407130d58f750c72c427a720c3db.ipa`,
  `source: original`; size 6,841,855 bytes.
- Hashes:
  - MD5: `646e407130d58f750c72c427a720c3db`
  - SHA-1: `04cc28a9104e7530c6786fc800737b3701093dbe`
  - SHA-256: `04e28cf42c25027ca196f3fdcff477152ba78d8ba6f8fa9aed0f11de03788c31`
- Embedded identity: display `Mr.AahH!!`, bundle `com.ponos.mraahh`, version
  `1.2.0`, minimum OS `3.0`. Runs 480x320 landscape.
- Options: none.
- tapHLEdb: App 17, version 17, report 25 (2026-07-26, tapHLE `f26722c0`,
  ★★★☆☆).

## Highest milestone: 3-star (In game), tapHLE `f26722c0`

Reproduced from a clean committed release build (window title
`Mr.AahH!! (tapHLE f26722c0)`, no `-dirty`).

Title screen -> first menu button -> gameplay. In gameplay the stick figure
hangs from a rope over pillars, tapping drops/swings him, and the loop
progresses: across two taps the level counter went **A-1 -> A-4** and the life
counter fell from **three hearts to one**. So levels advance and failure is
scored.

### Click map

No launch options; window 480x320 landscape; ~20 s title-screen wait.

1. Title -> `(378, 148)` first (topmost) menu button -> level A-1.
2. Gameplay -> `(240, 160)` taps -> the character drops; levels advance and
   lives are lost.

Note the buttons are unlabelled on screen (see below), so step 1 is "the
topmost of the three bars", not a named button.

## The only fix needed

`-[NSURLRequest initWithURL:]` was missing — only the three-argument
initialiser existed. That single addition (`74d745c5`) took the app from
aborting during startup to full gameplay.

## Known defect: menu buttons have no label text

**The three title-screen menu buttons render as blank dark bars.** They should
carry labels. This was spotted by the maintainer looking at the screen, not by
the log, which is the point: nothing warns about it.

What is already ruled out:

- **Not a font problem.** No font or glyph warning appears, and other text on
  the same screen renders correctly — the "MORE GAMES" button and the
  "© PONOS CO.,LTD" line are both legible.
- **Not localization.** The app ships `English.lproj/Localizable.strings` and
  `Japanese.lproj/Localizable.strings`, but no "Unable to locate localization
  table" warning is logged, so the table is being found. (Contrast Tap Tap
  Revenge 2, which *does* log that warning.)
- **Not missing assets.** The bundle contains 146 images named `img000.png`
  through `img145.png`, and no separately named button artwork. The label
  artwork is somewhere in that numbered set.

So the labels are almost certainly textures that are loaded but not drawn, or
drawn with wrong blending/coordinates. The bars do show a faint gradient, so
the button *background* draws while the label does not.

### Next discriminator

Enable `TAPHLE_LOG_MODULES=tapHLE::frameworks::opengles` and compare which
`img0NN.png` textures are uploaded against what appears on screen; or dump the
draw calls for the title screen and check whether the label quads are issued at
all. If they are issued, suspect the texture environment or blend mode; if they
are not, suspect the sprite that owns them never being added.

## Other known limitations at this rating

- Only the first menu entry was used; the other two were never opened, and
  because they are unlabelled it is not known what they are.
- Only world A was played, for a few levels. Whether a world can be completed
  is unknown.
- Audio was not measured.
- `___objc_personality_v0` and `_OBJC_EHTYPE_$_NSException` are unresolved at
  load; nothing has been observed to depend on them.
- `-[NSURLConnection initWithRequest:delegate:startImmediately:]` is a TODO
  returning nil, so the app's network call does nothing. It does not block
  gameplay.

## Checks run

- `cargo fmt --all -- --check`
- `cargo test --workspace --lib` (93 passed)
- `cargo build --release`

## 2026-07-27: 3 stars re-confirmed

Re-driven after a session of broad UIKit and CoreGraphics changes, using the
taps already recorded in this note — `(378, 148)` then `(240, 160)`. Two
captures eight seconds apart differ by SHA-256, so the loop is running.

Verified on a build of `0d8f5ec8`. The only source file changed between that and
`trunk` is `ca_eagl_layer.rs`, and that change adds a `log_dbg!` and nothing
else, so the emulator behaviour under test is identical to `trunk`.
