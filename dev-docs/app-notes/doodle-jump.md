# Doodle Jump compatibility work note

- Branch and starting commit: no `compat/doodle-jump` branch. Everything below
  was found and fixed on the framework branches listed under "What moved it",
  all of them merged to `trunk`; nothing about this game needed a change that
  was only good for this game.
- Embedded identity verified with `tapHLE --info` on `b7629c47`:
  - `Doodle Jump (v2.7.1) [Decrypted].ipa` — display name `DoodleJump`, bundle
    `com.yourcompany.DoodleJump`, version `2.7.1`, minimum OS `3.1`, iPhone.
    SHA-256 `80ca1e812164631f6015b8c5887402671f063adbb714e64cfba3839a343addb0`.
  - `Doodle Jump (v3.1.1) [Decrypted].ipa` — same display name and bundle,
    version `3.1.1`, minimum OS `3.1`, iPhone.
  - `Doodle Jump (v3.4) [Decrypted].ipa` — same display name and bundle,
    version `3.4`, minimum OS `3.1`, iPhone.
  - `Doodle Jump HD (v1.0) [Decrypted].ipa` — display name `DoodleJump`, bundle
    `com.limasky.doodlejumpipad`, version `1.0`, minimum OS `3.2`, iPad.
- **The iPhone builds and the iPad build are two different apps to the
  database**, because their bundle identifiers differ. The iPhone bundle
  identifier really is `com.yourcompany.DoodleJump` — Lima Sky shipped the
  Xcode template default — and it is not a placeholder introduced here.
- Route to gameplay: `dev-docs/clickmaps/doodle-jump.json`. Replay it rather
  than rediscovering the button.

## Where it stands (2026-08-18, tapHLE `d300e30b`)

- **v2.7.1: three stars.** Tapping Play from the menu reaches a live round —
  the doodler bouncing on a platform, platforms above and below, a monster
  across the top bar, the score, the pause control, the three tutorial
  captions. Still running ten seconds later, and two captures a few seconds
  apart differ: the doodler is at a different height and the monster has moved.
- **v3.1.1, v3.4, HD v1.0: two stars.** Each draws its complete main menu
  correctly. None has been taken past the menu yet; the route for v2.7.1 is
  known to work and the others have not been tried with it.
- The whole family was **one star** at the start of the day: every build died
  during start-up, and three different builds died in three different places.

## What moved it

Each of these was a framework gap that other apps share; none is a Doodle Jump
workaround. In the order the game hit them:

1. An unsigned long long in a property list could be held but not written, so
   writing the save ended the app (`fix/foundation-plist-unsigned-numbers`).
   All four builds stopped here.
2. `UIScrollView` had none of the six switches the game flips while building
   its theme carousel. It sets three of them in a row, so they were added as a
   set (`feat/uikit-scrollview-directional-lock`). v2.7.1 and HD.
3. `NSKeyedArchiver` could not be built around an app's own `NSMutableData`
   (`feat/foundation-archiver-into-mutable-data`). v3.1.1 and v3.4.
4. Writing a property list did not recognise `NSMutableData` as data, which is
   what the archive from (3) is (`fix/foundation-plist-mutable-subclasses`).
5. `NSKeyedUnarchiver` could not be told the decoding was finished
   (`feat/foundation-unarchiver-finish-decoding`). v3.1.1 and v3.4.
6. The game asks for accelerometer updates `inf` seconds apart — one divided by
   a frame rate of zero — and tapHLE tried to honour it, ending the app on the
   first tick (`fix/uikit-accelerometer-impossible-interval`). v3.1.1 and HD.
7. `CAKeyframeAnimation` did not exist
   (`feat/core-animation-keyframe-animation`). v2.7.1 and v3.4.
8. `NSValue` could not carry a `CATransform3D`
   (`feat/foundation-nsvalue-catransform3d`). v2.7.1 and v3.4.
9. `CAAnimationGroup` did not exist, and making groups work exposed a panic on
   any property the animation engine cannot animate — `transform`, here — which
   the same branch removed (`feat/core-animation-animation-group`). v2.7.1 and
   v3.4.
10. `-[UIWebView stopLoading]` did not exist, and the game stops its news web
    view at the moment a round begins, so it died exactly as gameplay started
    (`feat/uikit-webview-navigation-controls`). v2.7.1.

## The tilt problem, which is the reason this is three and not four

The doodler is steered by tilting the device. tapHLE reports no accelerometer
movement, so the doodler bounces on the platform it started on and the score
does not change. The round is alive rather than frozen — the physics run, the
monster moves, the doodler falls and bounces — but the player cannot go
anywhere.

This is the game's whole control scheme, so it is the discriminator for a
higher rating, and it is not a Doodle Jump problem: it is the same missing
input every tilt-steered game in the collection is waiting on. Anyone picking
this up should treat "give tapHLE a way to feed tilt" as the next piece of
work, not "make Doodle Jump climb".

## Open thread: the reports are not filed

The database at <https://taphle.ephun.net/compatibility> was returning
**HTTP 522** (Cloudflare could not reach the origin) on 2026-08-18 when these
results were ready, across repeated attempts to `GET /api/apps`. So:

- The **two-star** boundary for all four builds, crossed on `b7629c47`, is
  unreported.
- The **three-star** boundary for v2.7.1, crossed on `d300e30b`, is unreported.

Both were reproduced on the committed revisions named, with identity read from
`tapHLE --info`. Per `compatibility/README.md` a boundary passed without a
report cannot be filled in later from a note or a rerun on a newer revision —
so if the endpoint comes back and the tested revisions are no longer current,
submit **the rating the app holds now** and leave this paragraph as the record
of the gap rather than back-dating anything.

## Next discriminator

Take v3.1.1, v3.4 and HD past their menus with the recorded clickmap. The route
was recorded against v2.7.1; the other three lay out the same buttons but not
necessarily in the same places, and HD is an iPad build with a 1024x768 window,
so its coordinates are certainly different.
