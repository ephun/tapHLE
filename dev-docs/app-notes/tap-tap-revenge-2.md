# Tap Tap Revenge 2 compatibility work note

- Branch: `compat/tap-tap-revenge-2`. Reusable fixes graduated to `trunk`.
- Canonical artifact: <https://archive.org/download/ios3-6-ipas/com.tapulous.taptaprevengeII.ipa>
  (identifier `ios3-6-ipas`), `source: original`; size 11,862,689 bytes.
- Hashes:
  - MD5: `d9d1bcdfec1eac422fb198ef5ddc98be`
  - SHA-1: `cce3134212a8d6ce8fe6f76c38395c8ed838a68c`
  - SHA-256: `1de81bcc507a333468df1d2884142654d5591f9f32b1b7f205322c00e0f22186`
- Embedded identity (`tapHLE --info`): display name `Tap Tap`, bundle
  `com.tapulous.taptaprevengeII`, version `2.6.4`, minimum OS `2.0`, iPhone.
  The Archive filename carries no version; `--info` is the only source for it.
- Options: none. Window is 320x480 portrait.
- tapHLEdb: App 13, version 13. Report 21 (2026-07-26, tapHLE `102300c2`,
  ★★☆☆☆); report 43 (2026-07-27, tapHLE `b1de9e9e`, ★★★☆☆) supersedes it.

## Highest milestone: 3-star (In game), tapHLE `b1de9e9e`

Reproduced from a clean committed release build (window title
`Tap Tap (tapHLE b1de9e9e)`, no `-dirty`) against the hash-verified bytes.

Selecting a track loads the theme, parses the note chart and starts the audio
queue, and **the gameplay screen renders and plays**: notes fall down all three
lanes, the multiplier, streak and score readouts update, and the beat clock
drives the chart. Taps register — clicking a lane lights and enlarges its
target ring and changes the score. Eight captures taken four seconds apart
during play were eight distinct images.

Rated three, not more, because of the hang below. Three is
"Some gameplay works, but major problems remain", which is exactly this.

### Known limitation: it hangs after a minute or two of play

Play stops. The frame stops changing — five captures taken over ~50 s were
byte-identical, checked by SHA-256, not by eye — and touches stop registering,
while the process stays alive and the run loop keeps turning (the log continues
to grow). It is reproducible: it happened on both runs, at different scores
(-1,230 and -4,470) and with notes still mid-fall, so it is **not** the
song-failed transition, which was the first guess.

Nothing is logged at the moment it happens. That is the next thing to attack.

### Click map

No launch options; window 320x480 portrait. Startup got slower once views
started receiving a real layout pass, so allow **40 s** for the title screen.

1. Title -> `(160, 313)` Play -> Play menu. Allow 10 s.
2. Play menu -> `(82, 200)` One Player -> difficulty select. Allow 10 s.
3. Difficulty -> `(238, 190)` Easy -> track list. Allow 16 s.
4. Track list -> `(170, 128)` first track -> gameplay, after ~30 s of loading.

Every one of these taps is timing-sensitive, and a tap that lands early is
silently ignored — indistinguishable from a tap that missed. Capture between
steps rather than trusting the sequence.

Lane targets for tapping during play are at `y = 430`, `x = 55 / 160 / 265`.

## What it took: the layout pass

The last blocker was the interesting one, and it was general.

The game view is a standard `EAGLView`: `+[TTRGameView layerClass]` returns
`CAEAGLLayer`, and `-[TTRRenderer initWithContext:drawable:]` runs. But
`-[EAGLContext renderbufferStorage:fromDrawable:]` was **never called** — zero
hits when tracing that selector across a whole session — while
`presentRenderbuffer:` was called 887 times, each logging "renderbuffer 0 not
bound to a drawable". The renderer was initialised, believed it had a surface,
and presented every frame into nothing.

The cause is that the standard EAGLView creates its renderbuffer in
`-layoutSubviews`, and tapHLE only sent `layoutSubviews` at launch and for a
window's root view. A view added to a window hierarchy *later* never received
one. UIKit lays out every view in a window on the next turn of the run loop, so
that was simply missing; it is fixed on `trunk`, and the pass is deliberately
skipped for a view not yet in a window, because laying out before the view has
its final size would create the renderbuffer at the wrong size and nothing
would re-create it.

Worth noting how misleading the symptom was: an app that runs, loads its data,
plays audio and draws a blank screen looks like a rendering bug in the
emulator's compositor. The 887 harmless-looking log lines were the whole
answer.

### Twelve general gaps cleared to get here

All on `trunk`; none is specific to this game. In the order they were hit:

1. `-[NSDictionary initWithContentsOfFile:]` passed a nil path straight to
   `to_rust_string()`. Both concrete dictionary classes had the gap.
2. `UIGraphicsBeginImageContext` and friends — assembled from the existing
   `CGBitmapContext` and the UIGraphics context stack, not new drawing code.
3. `-[CALayer renderInContext:]`, deliberately partial: background colour and
   `contents` only, no transforms or masking, and it says so.
4. `-[NSMutableString initWithContentsOfFile:]` — the sibling-class trap again.
5. `CC_MD5_Init`/`_Update`/`_Final`.
6. `object_getInstanceVariable` and its setter.
7. Declared-property metadata: `class_getProperty` was a stub returning null.
8. Return values for `NSInvocation` — `-invoke` asserted the return type was
   void.
9. `AudioQueueGetCurrentTime`, `AudioQueueEnqueueBufferWithParameters`'s
   `outActualStartTime`, and `kAudioFilePropertyPacketToFrame`.
10. `CFRunLoopTimerCreate` asserted `order == 0`; `MPVolumeSettingsAlert*`;
    `-[NSCalendar components:fromDate:toDate:options:]`.
11. `viewDidLoad` was being sent too widely — see below.
12. The layout pass above.

### The Lua bridge

The app's theme is Lua, and it failed with

```text
[string "theme.cfg"]:1044: attempt to compare function with number
Error setting up taps: no columns
```

Line 1044 of the app's own `game_defaults.cfg` (loaded under the chunk name
`theme.cfg`) reads `game.gameController.currentFrameRate < 16`. The bridge
resolves a property with `class_getProperty`, which tapHLE stubbed out to
return null, so it fell through to handing Lua a *bound method* instead of the
value — hence comparing a function with a number. With property metadata parsed
from the binary the chain `currentFrameRate` -> `gameView` -> `view` ->
`framesPerSecond` resolves and the theme loads clean.

The binary's imports named the mechanism before any tracing did:
`class_getProperty`, `property_getAttributes` and `object_getInstanceVariable`
together are a scripting bridge, and all three were missing or stubbed.

### A regression this session introduced, and the rule behind it

`-[UIViewController view]` was sending `viewDidLoad` whenever the view was
non-nil. That is wrong, and this app is what proved it: `TTRGameController`
builds its OpenGL view by hand and calls `-setView:`, and its `viewDidLoad` is
a **teardown** — it sent `unloadResources`, `removeFromSuperview` and
`setView:nil`, destroying the game view microseconds after it was created.

The rule is that `viewDidLoad` reports *loading*, so it belongs on exactly two
paths: after `-loadView` runs, and after a nib supplies the view. A controller
whose view the app assigned itself never loaded one and must not be told it
did. Glass Tower HD, which is what motivated sending `viewDidLoad` in the first
place, still reaches gameplay after the narrowing — verified, not assumed.

## Earlier gaps, cleared before the twelve above

All on `trunk`; none is specific to this game. In the order they were hit:

1. `-[NSBundle load]` and friends — a resource-only `.bundle` (FBConnect's
   images) has no code to load and is trivially loaded. (`3abf3073`)
2. `+[NSBundle bundleForClass:]`. (`3abf3073`)
3. `NSNetServiceBrowser` / `NSNetService` — no Bonjour, so both report the
   documented failure rather than leaving the app waiting. (`3abf3073`)
4. `-[NSString initWithBytesNoCopy:length:encoding:freeWhenDone:]`.
   (`3abf3073`)
5. `NSURLRequest`'s missing accessors and mutable setters. (`3abf3073`)
6. `-[UIApplication setNetworkActivityIndicatorVisible:]`. (`3abf3073`)
7. `NSSearchPathForDirectoriesInDomains()` asserted the user domain.
   (`3abf3073`)
8. `+[CATransaction setValue:forKey:]` unwrapped a missing implicit
   transaction. (`3abf3073`)
9. `NSSortDescriptor`, `NSCountedSet`, `NSData
   dataWithContentsOfFile:options:error:`, keyed-unarchiver `Data`/`Boolean`
   leaves, a real `UIAlertView` dismissal, optional `CAAnimationDelegate`
   methods, `CFDictionaryCreateMutable` capacity as a hint, and an unknown
   `AudioServicesPlaySystemSound` ID. (`d51d051b`)

### Two lessons worth carrying forward

**Sibling concrete classes do not inherit from each other.**
`_tapHLE_NSMutableArray` descends from `NSMutableArray`, not from
`_tapHLE_NSArray`, and the same holds for `_tapHLE_NSMutableString` vs
`_tapHLE_NSString`. Anything `NSArray`/`NSString` declares has to be added to
**both** concrete classes. This cost three separate rebuild cycles in this
session (`initWithObjects:`, `initWithBytesNoCopy:...`,
`sortedArrayUsingDescriptors:`) and also revealed that
`-sortedArrayUsingSelector:` had been missing from the mutable array all along.

**Optional delegate methods must be probed.** Sending a Cocoa delegate protocol
method blind turns a delegate that implements only the callbacks it cares about
into a "does not respond to selector" abort. `CAAnimation` was doing this.

## Known limitations at this rating

- No gameplay at all; the note chart never loads.
- Two Player, Career and Play Online were not tried. Online will not work:
  tapHLE has no network stack for Tapulous' services, and the app's own
  Bonjour-based local multiplayer is answered with "no network".
- The welcome alert is auto-dismissed via cancel rather than shown, so its
  "import your existing profile" branch is never taken.
- Audio was not measured. Two system-sound IDs are played that tapHLE never
  created, and are ignored.

## Checks run

- `cargo fmt --all -- --check`
- `cargo test --workspace --lib` (93 passed)
- `cargo build --release`
