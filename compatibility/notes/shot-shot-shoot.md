# Shot Shot Shoot compatibility work note

- Branch: `compat/shot-shot-shoot`. Reusable fixes graduated to `trunk`.
- Canonical artifact: <https://archive.org/details/ios-ipa-com.eriksvedang.shotshotshoot>,
  file `Shot Shot Shoot (v1.0) [Decrypted].ipa`, `source: original`; size
  27,316,931 bytes. (The item holds four names for the same bytes — v1.0/v1.01
  and Decrypted/Cracked all share one MD5.)
- Hashes:
  - MD5: `afe545b4cb8b62c491c4f4f87113e3ee`
  - SHA-1: `6cb4022fc1d48cbb1f36d779b08b5902479bf44b`
  - SHA-256: `7d750072ee7d4ea90c644c58babbdd16cf4463e6ae2bdb0dd5600f240d8c0a09`
- Embedded identity: display `ShotShotShoot`, bundle
  `com.eriksvedang.shotshotshoot`. An openFrameworks game.
- Options: none. Client window is 1024x768.
- tapHLEdb: App 19, version 19, report 27 (2026-07-26, tapHLE `730f2c29`,
  ★★★☆☆).

## 2026-08-13: regressed. The menu draws, the text is a quarter turn out

Reported to the database as two stars. The menu reaches a stable screen and the
button layout is landscape-correct and unstretched — the circles are circles, so
nothing is being squashed — but every label is rotated a quarter turn and START
is rotated further, so none of it reads.

**Not caused by the compositing and presentation work of the same day.** On
`656e01ab`, the revision immediately before that work, this app renders a blank
white client area and nothing else, for at least ninety seconds. So it was
already regressed, and those fixes moved it from drawing nothing to drawing a
menu. The regression is somewhere between `730f2c29` (2026-07-26, the three-star
revision) and `656e01ab`, and finding it needs a bisect over that range.

What is measured so far:

- `Info.plist` declares `UIInterfaceOrientation = UIInterfaceOrientationPortrait`
  and ships no `UISupportedInterfaceOrientations`.
- tapHLE logs `applying device orientation Portrait`, and
  `Window::rotation_matrix` is the identity for Portrait, so **no presentation
  rotation is being applied**.
- The client area is nevertheless 1024x768 landscape, and the content fills it
  at the right aspect ratio.

### The lead: `setStatusBarOrientation:` rotates the device, and should not

`-[UIApplication setStatusBarOrientation:]` in `ui_application.rs` calls
`window.rotate_device(...)`. On iOS that call does not rotate anything: it tells
UIKit which way the status bar faces, and the app stays responsible for its own
rendering orientation. openFrameworks calls it, and this is an openFrameworks
game.

That is the only silent path that can produce what is measured here, and the
measurements fit it exactly:

- The launch log shows tapHLE starting the app portrait, from the `Info.plist`.
- The view-controller autorotation added in `126983ce` does not fire: its
  `log_dbg!` never appears, and tapHLE's default
  `shouldAutorotateToInterfaceOrientation:` correctly accepts portrait, which
  ends that path before it rotates anything.
- The client area is 1024x768 anyway, and `size_for_orientation` can only
  produce that from a *landscape* device orientation.

So something rotates the device after launch without logging, and
`setStatusBarOrientation:` is the only candidate. The app then rotates its own
scene as well; the two agree for everything drawn through the app's transform,
which is why the layout is landscape and correctly proportioned, and disagree
for anything that is not, which is why the labels sit a quarter turn out.

**Before changing it, note the blast radius.** Every app that calls
`setStatusBarOrientation:` currently gets a device rotation out of it, and some
of them presumably look right *because* of that. Making the call a no-op is a
shared-path change and needs a full `dev-scripts/regression-sweep.ps1` run
compared against a previous one, not a check against this app alone. That is
the mistake this note's sibling entries already record twice.

Forcing an orientation from the command line is not a workaround: with
`--landscape-left` the app does not survive to a frame at all.

Those three together are the discriminator to chase: an app that renders
landscape into a window tapHLE believes is portrait, with the labels rotated
relative to the layout rather than with it. Whatever rotates the text is doing
it independently of the frame, so look for a text path with its own transform
before looking at the presentation path again.

The coordinate warning below still applies and is probably related.

## Highest milestone: 3-star (In game), tapHLE `730f2c29`

Reproduced from a clean committed release build (window title
`ShotShotShoot (tapHLE 730f2c29)`, no `-dirty`).

Menu renders with START and the four mode buttons (Tutorial, Player vs Player,
Computer Easy, Computer Hard). START enters the tutorial level. Tapping the
shoot region fires: shot trails rise, ammo squares in the middle row are
consumed, the ammo bar shortens, and the centre enemy block is destroyed.
Colours are randomised per run, so the same level looks different each time.

### Click map

No launch options; client window 1024x768; ~22 s menu wait.

1. Menu -> `(512, 384)` START -> tutorial level.
2. Level -> `(800, 384)`, `(880, 384)`, `(950, 384)` -> shots fire.

**Coordinate warning.** The frame capture is the pre-rotation EAGL renderbuffer
and is 768x1024 *portrait*, while the client area is 1024x768 *landscape*. So a
position read off a capture is not a client coordinate: client X maps from
capture Y. Centre maps to centre, which is why START worked immediately, but
the shoot region — near the bottom of the capture — is at high client X, not
low client Y. Three taps at low X did nothing before this was worked out.

## Fixes this required

Both general, both on `trunk` in `32ac92f0`.

1. `-[NSString initWithContentsOfFile:encoding:error:]` lacked the nil-path
   guard its single-argument sibling already had, so a nil path reached
   `to_rust_string()` and panicked. It also asserted that the caller had *not*
   passed an `NSError**`, which would abort any app that did. Both fixed, and
   the NSURL variant got the same treatment.
2. `ObjC::borrow`/`borrow_mut` used a bare `unwrap()` on the object-table
   lookup, so a miss reported only "called `Option::unwrap()` on a `None`
   value" — no object, no type, no lead. It now names both and states the two
   usual causes. **This is what found bug 1**: the improved message showed the
   object was literally nil and the wanted host type was `StringHostObject`,
   which pointed straight at the string initialiser.

Tap Tap Revenge 2 fails at the same `objects.rs` site when loading a track, so
that app should be re-run now that the message is informative — it may be a
different cause, but it is no longer blind.

## Known limitations at this rating

- Only the tutorial was played, and only far enough to destroy one block.
  Whether the tutorial can be completed is unknown.
- Player vs Player, Computer Easy and Computer Hard were never opened.
- Steering ("swipe left and right") was not exercised, only tapping.
- `Unable to load settings.xml check data/ folder` is printed at startup; the
  game continues with defaults.
- Audio was not measured. `SoundEngine initialized` is logged.

## Checks run

- `cargo fmt --all -- --check`
- `cargo test --workspace --lib` (93 passed)
- `cargo build --release`

## 2026-07-27: 3 stars re-confirmed

Re-driven after a session of broad UIKit and CoreGraphics changes, using the
taps already in this note — `(512, 384)`, then `(800, 384)`, `(880, 384)`,
`(950, 384)`. That reaches the interactive tutorial, which is playable: the
target row, the ammo squares, the shooting region and both ammo gauges all
render, and a projectile is in flight. Two captures eight seconds apart differ
by SHA-256.

**The capture is small — about 26 KB — and that is not a warning sign here.**
This game's art is flat colour on white, so it compresses to a fraction of what
a photographic title produces. Judging health by capture size would flag this
app as blank; look at the image.

Verified on a build of `0d8f5ec8`, whose only source difference from `trunk` is
a `log_dbg!` added to `ca_eagl_layer.rs`.
