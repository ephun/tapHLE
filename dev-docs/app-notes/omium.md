# Omium (2 Player Shooter) compatibility work note

- Branch: `compat/omium`.
- Canonical artifact: <https://archive.org/download/ios-ipa-collection/>,
  `Omium.2.Player.Shooter.v1.0.ipa.ipa` (the doubled extension is the Archive
  filename), `source: original`; size 1,631,044 bytes.
- Hashes:
  - MD5: `914522510ea8e30d0c8d56bcb3c88c85`
  - SHA-1: `396a22fa9c342774f6d2bc6ecd2e5ee7d58ab2e0`
  - SHA-256: `e9bf9b9be42424d1b70e0f11e9b15ec499b1caf2a8cd4f525228e926966d33f8`
- Embedded identity (`tapHLE --info`): display `Omium`, bundle identifier
  **`com.eeenmachine.`** — with a trailing dot and no app segment. Version
  `1.0`, iPad. Same developer as Scoops (`com.eeenmachine.scoops`).
- tapHLEdb: App 25, version 25, report 33 (2026-07-26, tapHLE `9d6ee348`,
  ★★☆☆☆). Report 32 (app 24) is a **bad submission by the same agent** that
  guessed `com.mrlacey.omium`; it should be rejected in moderation.

## Highest milestone: 3-star (In game), tapHLE `6b4a2811`

Dodge mode plays. From the menu, `(384, 631)` selects Dodge and `(384, 512)`
("Press here to play") starts it; the round then runs with the ship, the AMMO
and BAD GUYS gauges, particle effects and a **counting timer** (11.68 at one
capture, with consecutive captures differing in size — the check that separates
a live round from a static card).

Note the screen is half upside-down by design: this is a two-player game with
the players at opposite ends of the iPad. That is not a rendering fault.

> **Report not filed.** tapHLEdb returned HTTP 504 on every attempt. The result
> above is measured on clean commit `6b4a2811` and should be submitted as
> **3 stars** when the endpoint recovers. App 25 already exists under
> `com.eeenmachine.`; report 33 (2 stars) predates the fix and is superseded by
> this.

## Superseded: the 2-star reading, tapHLE `9d6ee348`

The menu renders and is live: the word "OMIUM" drawn entirely out of moving
point-sprite particles, over the entries Dodge, Juggle, Infinite and More
Games. Two captures seconds apart show the particle field in different
positions, so the app is animating, not frozen.

It launched with **no new fixes at all** — the point-sprite work done earlier
for Glass Tower 3 (`GL_POINT_SIZE_ARRAY_OES` reported as `GL_INVALID_ENUM`
instead of asserting, and `glPointSizePointerOES` accepted) is exactly what
this app needs, and both log on startup here.

## Root cause, now fixed: the startup orientation override

The menu could not be tapped because **input was vertically inverted relative
to the display**, measured on a 768x1024 client as `delivered_y = 1024 - y`
(631 arrived as 393; 200 arrived as 824).

The cause was in `Environment`'s startup orientation selection, not in the
touch path. It asked "does this app list any non-portrait orientation?" and, if
so, rotated the device to the first one found. Omium declares
`['UIInterfaceOrientationPortrait', 'UIInterfaceOrientationPortraitUpsideDown']`,
so it was rotated 180 degrees away from an orientation it already supported.
Rotating the device rotates input as well as display, and the two are derived
separately, so every touch arrived mirrored.

Fixed in `752203a9`: the override now applies only when the app genuinely
cannot do portrait. Landscape titles are unaffected because they do not list
portrait, so they take the same path as before — verified by a regression sweep
over eight apps, six with byte-identical frames.

## Superseded frontier: touch input is vertically inverted

Menu taps do not select, and a `TAPHLE_LOG_MODULES=tapHLE::frameworks::uikit::ui_touch`
run explains why. **The touch is delivered, but at the wrong place.** Two
measurements, on a 768x1024 client with a 768x1024 `EAGLView`:

| clicked client y | delivered to app |
|---|---|
| 631 | 393 |
| 200 | 824 |

Both satisfy `delivered_y = 1024 - client_y`. The y axis is **flipped**.

This is not a coordinate mistake in the click map: hit-testing finds the app's
`EAGLView` and `touchesBegan:`/`touchesEnded:` are both delivered, so routing
works. The point itself is mirrored, which is why tapping "Dodge" lands
somewhere near the title instead.

The display is **not** mirrored — the menu renders right way up, with "OMIUM"
above the entries and "More Games" at the bottom. So input and output disagree
about the vertical axis. That is an emulator bug, not an app quirk, and it
would affect any app tapHLE puts in the same orientation.

x could not be measured: both usable taps were at x=384, the exact horizontal
centre, where a flip is invisible. **Measure x before concluding this is a
simple vertical flip rather than a 180-degree rotation** — an iPad app whose
`Info.plist` allows `UIInterfaceOrientationPortraitUpsideDown` would plausibly
get a 180 rotation applied to input only, and that would look identical at
x=384.

### Next discriminator

One run, three clicks, all off-centre in x — say `(200, 300)`, `(600, 300)`,
`(200, 700)` — reading the delivered point from the `ui_touch` log each time.
If x also mirrors (`768 - x`), it is a 180-degree rotation; if x passes
through, it is a pure y flip. Then find where tapHLE derives the input
transform and compare it against the one used for presentation; they are
evidently not the same for this orientation.

Verifying the fix is cheap: with input and display agreeing, tapping "Dodge" at
`(384, 631)` should enter the mode, taking this app to 3 stars. Nothing else is
known to be wrong with it — it needed no fixes at all to render.

Since point sprites are drawn at a uniform size here (tapHLE does not model
`GL_POINT_SIZE_ARRAY_OES`), the particle title may also be visually wrong in a
way that matters to hit-testing if the app picks by particle proximity.

## 2026-07-27: 3 stars re-confirmed

Re-driven after a session of broad UIKit and CoreGraphics changes. The
instruction screen renders — the ship, the AMMO gauge, the mirrored two-player
layout and a `0.00` timer — and **`Press here to play` is at `(384, 512)`**, not
the `(384, 631)` this note listed first. Tapping it starts the round: two
captures eight seconds apart differ by SHA-256.

The stale first coordinate cost a run. Both are now listed in the order to try
them, since the wrong one leaves the app sitting on the instruction screen
looking exactly like a game that will not start.

Verified on a build of `0d8f5ec8`. The only source file changed between that and
`trunk` is `ca_eagl_layer.rs`, and that change adds a `log_dbg!` and nothing
else, so the behaviour under test is identical to `trunk`.
