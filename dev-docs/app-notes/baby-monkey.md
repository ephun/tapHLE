# Baby Monkey compatibility work note

Last updated: 2026-07-23. Rating: ★★★☆☆ (3/5) In game — see the last section
for what works, what does not, and where to resume.

## Identity and source

- Work branch: `compat/baby-monkey`.
- Canonical Archive item:
  <https://archive.org/details/ios-ipa-com.kihon.babymonkey>.
- Maintainer-designated newest filename:
  `Baby Monkey (going backwards on a pig) (v1.3.5) [Decrypted].ipa`.
- Archive original size: 9,947,604 bytes.
- Archive MD5: `cf3eaa2326db3d2614b589f4e438312b`.
- Archive SHA-1: `02d70acdac7c8cd79640dab2336a1eaaf1382a5f`.
- Locally calculated SHA-256:
  `5ae6373838f9cff4a8a2001bc30253be88f0fb05f707af8c7bf8259330cd3346`.
- Embedded bundle: `com.kihon.babymonkey`, version/build `1.01`, minimum OS
  `4.2`.

Archive metadata was live-verified on 2026-07-19 and the downloaded bytes
match its exact original size, MD5, and SHA-1. `tapHLE --info` identifies the
embedded app version as `1.01`, not `1.3.5`. The Archive originals named
`v1.01`, `v1.2.3`, and `v1.3.5` are byte-for-byte identical. The local upload
manifest explains the mistake: all three destination names point to the same
`BabyMonkey (v1.01) [Decrypted].ipa` source. Keep the requested highest-version
Archive filename as the canonical source filename, but report the embedded
version as authoritative and do not claim that these bytes contain 1.3.5.

An earlier local file at the same picker path was 21,978,076 bytes with
SHA-256 `c1879dc8177f57ae0587847e2d0aadad264c0a4bf502bb77bdb74a6d3f654693`.
It did not match any decrypted Archive original. It is preserved with a
non-IPA extension under ignored `tapHLE_apps/_quarantine` and must not be run
or cited as compatibility evidence.

Availability was checked on 2026-07-19. Apple's public Lookup API returned no
result for App Store ID `447960108` in the United States and 23 other sampled
storefronts, and the former US product URL returned HTTP 404. Kihon's surviving
games page is historical and its app-specific site was unavailable. No current
official purchase or download route was found. This is a bounded availability
observation, not proof of legal abandonment or universal unavailability.

## Clickmap

`dev-docs/clickmaps/baby-monkey.json` replays launch to a live run. Run it
with `dev-scripts/clickmap.ps1` before working out a route by hand.

## Current canonical checkpoint

The release build for commit `ee21050a` was launched in a normal Windows
window with the exact hash-verified Archive bytes. It loads the armv7 slice,
selects landscape orientation, creates an OpenGL ES 1.1 context through the
GLES1-on-GL2 layer, creates the app's EAGL context, and obtains the host country
and preferred language. No app-rendered frame was established before it
exited.

The canonical stop was a null-page read at guest PC `0xc1010`. Static Mach-O
analysis proved that the preceding instructions load non-lazy slot `0xf4688`,
which maps to the unresolved `_NSLocaleLanguageCode` import. The containing
guest method is `-[CBNetwork makeRequest:params:]`; it calls
`[NSLocale currentLocale]` and then asks it for that key.

Commit `7dffbc97` exports `_NSLocaleLanguageCode` as an NSString constant and
returns the stored locale language for both the Foundation and Core Foundation
language-code keys. Its exact release build passed PC `0xc1010`, constructed
the Chartboost install/get requests, and reached Flurry startup. The language
symbol is no longer unresolved.

The next crash sends `setObject:forKey:` to an immutable
`_tapHLE_NSDictionary`. The app reached this state because
`+[NSDictionary dictionaryWithObject:forKey:]` is inherited by
`NSMutableDictionary` and returns `instancetype`, but tapHLE's implementation
forced allocation through the immutable NSDictionary concrete class. The
current branch allocates through the receiving class and shares the existing
key/value initialization path, so a call through NSMutableDictionary produces
the mutable concrete class. All 78 release library tests and
`cargo check --release` pass. This class-cluster correction still needs an exact
committed release build and visible canonical-IPA launch before the next
runtime frontier is claimed.

## Menu render checkpoint (2026-07-21)

Building on the dictionary correction, the `compat/baby-monkey` branch advanced
Baby Monkey through several further boundaries to a fully rendered, human-visible
main menu. The observed sequence of frontiers and the fix for each:

1. `initWithDictionary:nil` / `addEntriesFromDictionary:nil` panicked. Commit
   `e74f836f` treats a nil source dictionary as empty.
2. The unresolved non-lazy import `kCFTypeArrayCallBacks` dereferenced null.
   Commit `efdd1394` exports it as a real callbacks struct, makes
   `CFArrayCreateMutable` honour the callbacks' retain slot, and adds
   `CFArrayGetFirstIndexOfValue` (the subsequent unimplemented import).
3. `setValue:@(volume)forKey:@"volume"` on `KataCCBackgroundMusicBehavior` hit a
   hard NSNumber assert. Commit `56ce209d` unwraps a boxed scalar into the
   setter's encoded primitive type during key-value coding.
4. The metaclass received the single-argument
   `cancelPreviousPerformRequestsWithTarget:`. Commit `730b0246` adds it,
   cancelling every queued perform request for the target.

After these, the exact working tree now committed at `f71fbdcd` boots without
panicking or segfaulting: it creates the GLES1-on-GL2 context, sets up the
AVAudioSession, initialises the Chartboost/Flurry SDK stubs, opens OpenAL, and
renders the interactive main menu (Play button, monkey and pig sprites,
parallax background, and the four bottom-bar buttons). This is the first
app-rendered frame established for Baby Monkey.

Two known limitations at this checkpoint:

- The menu was observed from a release binary built on the identical source now
  committed at `f71fbdcd`, but the window title reported the pre-commit dirty
  tree (`eb6397f7-dirty`). A clean rebuild at `f71fbdcd` should reproduce the
  menu before any formal compatibility-database entry or star rating is added.
- `AudioFileOpenURL()` fails to load the menu track: the game passes the
  relative path `audio/YaYaYaYa.wav`, which does not resolve against the guest
  working directory (`/`). **"Menu audio is therefore silent" was an inference,
  not an observation, and it is wrong** — see the 2026-07-23 section, where a
  wave capture shows continuous audio in the menu. The failing loads are real;
  the silence conclusion drawn from them was not checked.

## Gameplay-load investigation (2026-07-21)

Clicking the rendered menu's Play button (via process-scoped foreground input)
dismisses the menu and begins loading the gameplay scene. That load hit a
sequence of frontiers, each fixed with a reusable behavior:

1. `-[NSMutableSet objectsPassingTest:]` was unimplemented. Now implemented via
   a new reusable `objc::blocks::block_invoke_function` helper.
2. `CGRectFromString(nil)` panicked in `to_rust_string`. The `*FromString`
   geometry parsers now return zeroes for a nil string, matching Apple's
   documented not-well-formed behavior.
3. `CFArrayInsertValueAtIndex` was unimplemented (added, with
   `CFArraySetValueAtIndex`).
4. Current frontier: the game dispatches its custom `onTouchesBegan:` selector
   to a responder list that contains an `NSString`, which panics. `respondsTo
   Selector:` is correct, so the game is not filtering — the list is genuinely
   corrupt.

Root cause (evidence-backed, not yet fixed): the gameplay scene is a black
screen because its object graph is being decoded with missing/wrong values. The
game is a **custom engine** whose `.prefab`/`.scene` files are **XML property
lists that are `NSKeyedArchiver` archives**, decoded through `NSKeyedUnarchiver`
+ guest `initWithCoder:` (all of `NSKeyedUnarchiver`, `unarchiveObjectWith...`,
`initWithCoder`, `NSCoding` appear in the binary). tapHLE has an
`NSKeyedUnarchiver`, and the simpler `MainMenu.prefab` decodes well enough to
render the menu, but the richer gameplay prefab exposes decoding gaps that yield
`nil` rects and `NSString`s where game objects belong — producing both the nil
`CGRectFromString` and the `NSString`-in-responders panic. The next
discriminator is to instrument the keyed-unarchiver decode path and find which
`decodeObjectForKey:`/reference resolution returns the wrong value for the
gameplay archive.

Separately (non-fatal): the game's engine locates audio correctly under
`bm/audio/` (an existence probe for `.../BabyMonkey.app/bm/audio/YaYaYaYa.wav`
returns true), but the sound engine loads via a `pathForResource`-style lookup
that searches `<resourcePath>/audio/` (bundle root, not `bm/`) and misses, so
every sound fails with `FileReadError`. The game survives without audio. This is
a genuine engine/path quirk of this build, not the fatal blocker.

Rating at this checkpoint remains **★★☆☆☆ (2/5) Starts** — an interactive menu
works, gameplay does not yet render. Reaching 3/5 In game requires completing
the keyed-unarchiver decoding of the gameplay object graph.

Follow-up (same day): a first, general unarchiver bug was fixed — a UID
reference to `$objects[0]` (the "$null" placeholder) was being decoded as a real
`NSString "$null"` instead of nil, corrupting every nil property/element/handler
in the graph. `unarchive_key` now returns nil for UID 0. That cleared the
`onTouchesBegan:`-on-`NSString` panic and the process now survives clicking Play
without that crash, but the gameplay scene is still black and now panics with
`onTouchesBegan:` sent to an `_tapHLE_NSArray`.

The gameplay scene is `bm/GameScene.scene`, a 56 KB XML-plist keyed archive for
the game's own component engine ("Kata"): ~85 `KataUserBehaviorBase` behaviors
plus `KataCCAnimationBehavior`, `KataCC*`, `bm*`, and ~30 `*Alias`
(`kSpatialBehaviorAlias`, `kImageBehaviorAlias`, `kSpriteBehaviorAlias`)
instances. The scene crashes during behavior-graph setup before its first frame,
which is why the screen is black. Getting to 3/5 is therefore a bounded but
substantial effort: complete the keyed-unarchiver + guest `initWithCoder:`
decoding of this behavior graph so no behavior slot resolves to a wrong-typed
object (`NSArray`/etc.). Next discriminator: instrument `unarchive_key` to log
the decoded class per UID and trace which behavior/alias reference resolves to
an array where a `Kata*` behavior is expected.

## Earlier noncanonical work

The branch also contains reusable emulator work developed while the mismatched
21.9 MB local file was mistakenly treated as the target. That work includes
Foundation/Core Foundation behavior, Objective-C blocks and initialization,
dispatch queues, native ES2 and iOS GL extension support, UIKit controller
mounting, standard-stream routing, `NSProcessInfo.environment`, and Darwin
`sigaltstack` behavior.

The implementations have their own automated checks, and the signal-stack
work received a focused ABI/semantics review. However, the old file's progress
through display-loop setup and `_sigaltstack` is not Baby Monkey compatibility
evidence. Revalidate relevant milestones against the canonical Archive bytes
instead of continuing from the discarded file's last log.

The native ES2 provenance recorded in older branch documentation was also too
imprecise: HyperHLE commit `ec06f12b` is a later tree snapshot, not the
originating ES2 change. The implementation first appears in HyperHLE commit
`d640dd4ddba1deb4c5eac9761239921bb3245601`, authored by Бусик and co-authored
by Devin AI. Preserve this correction prospectively without rewriting the
published tapHLE commit.

## Standing run rules

Do not use `--headless`: UIKit/EAGL startup needs a real tapHLE window and a
headless unwrap failure would be unrelated. Do not add a compatibility report
or star rating until a human-visible milestone is reproduced from a committed
build using these exact canonical bytes.

## Verification performed

- Archive metadata and the exact downloaded original agree on filename, size,
  MD5, and SHA-1; SHA-256 was calculated locally.
- `tapHLE ee21050a --info` reports version `1.01`, bundle
  `com.kihon.babymonkey`, minimum OS `4.2`, and iPhone device family.
- Static armv7 disassembly, indirect symbols, and runtime registers all agree
  on the `_NSLocaleLanguageCode` null dereference.
- The focused locale export test passes.
- The exact `7dffbc97` windowed run passes the locale dereference and reaches
  the dictionary class-cluster error described above.
- The release library test binary passes all 78 tests.
- `cargo check --release` passes with the existing GLES dead-code warning
  group.
- The full integration test remains unavailable because the reviewed custom
  test SDK and `tests/llvm/bin/clang.exe` are not installed; the failure is an
  environment prerequisite, not an app result.

## Correction and root cause of the black gameplay screen (2026-07-22)

**The 2026-07-21 root-cause claim above is wrong and must not be resumed.** It
states that `bm/GameScene.scene` is "a 56 KB XML-plist keyed archive" and that
reaching 3/5 requires completing `NSKeyedUnarchiver` decoding of the behavior
graph. It is not a keyed archive. Every one of the 131 `.scene`/`.prefab` files
in the bundle is a plain XML property list with the engine's own schema
(`archetypeName`, `name`, `behaviors`, `children`); none contains `$objects` or
`$archiver`. They are read with `dictionaryWithContentsOfFile:` and walked by
the engine, so `NSKeyedUnarchiver` is not in this path at all. An agent that
resumes the old discriminator will instrument code the scene never executes.

The `$null`/UID-0 unarchiver fix (`33527851`) is still a correct general fix —
a UID reference to `$objects[0]` does mean nil — but it is unrelated to this
app's gameplay load, and the claim that it "cleared the `onTouchesBegan:`
panic" was a misreading of nondeterministic output.

### Actual root cause: use-after-free in the touch-responder list

`KataCCButtonBehavior` instances are deallocated while the engine's touch
dispatcher still holds pointers to them, so `onTouchesBegan:` is sent to
whatever object later occupies that address. Direct evidence from one run:

```
dealloc KataCCButtonBehavior 0x3024bf00
... later, same run ...
Object 0x3024bf00 (class "_tapHLE_NSDictionary") does not respond to
selector "onTouchesBegan:"!
```

Four runs of the identical committed build produced four different receiver
types at the same point — `_tapHLE_NSString "density"`, `_tapHLE_NSString
"kSpatialBehaviorAlias"`, `_tapHLE_NSDictionary`, and `_tapHLE_NSArray`. A
deterministic scene load cannot yield a nondeterministic receiver type from a
decoding bug; it is the signature of freed memory being reallocated. The
specific strings are incidental (a Box2D fixture key, a `classAlias` value) and
chasing their meaning is a dead end.

148 `Kata*`/`bm*` objects are deallocated during the gameplay-scene load,
including 19 `KataCCSpatialBehavior`, 16 `KataCCImageBehavior`, and 5
`KataCCButtonBehavior`. No object was reported as deallocated with a non-zero
refcount, so this is a missing retain (or a retain the engine expects a
container to perform), not a double release.

### Ruled out, with evidence — do not re-test these

- **Alias registry is populated.** `mapAlias:toClass:` is called 53 times on
  `KataSVUserBehaviorController`, including all three of
  `kSpatialBehaviorAlias`, `kImageBehaviorAlias`, `kSpriteBehaviorAlias`.
  `getClassForAlias:` resolves against a populated table.
- `NSDictionary` fast enumeration yields keys (uses `allKeys`); `allKeys` and
  `allValues` map to the correct halves of each pair.
- `NSString`'s `hash` and `isEqual:` are on the base class and compare
  contents, so binary constant strings and plist-parsed strings match as
  dictionary keys.
- `+load` is implemented and dispatched from `environment.rs`.
- `removeObserver:` with a nil name and object removes every matching
  registration. **The second half of this bullet was wrong** — it said the touch
  path does not use `NSNotificationCenter`. It does; see the 2026-07-23 section.
- The autorelease pool in `ui_touch.rs` correctly wraps the whole
  `touchesBegan:` delivery and drains only after every view has been messaged.

### Next discriminator

In the failing run the fatal release of a `KataCCButtonBehavior` is logged
*before* the surviving buttons receive `onTouchesBegan:`, so the object dies
during scene setup/teardown rather than mid-dispatch. Find what is supposed to
own it: instrument `retain`/`release` for one `KataCCButtonBehavior` from
`alloc` onwards, recording the guest `LR` at each call, and identify which
retain that iOS performs is missing under tapHLE. The guest frame pointer chain
is not walkable from inside a host call, so capture `LR` at the call site
rather than relying on `dump_current_guest_state`'s stack trace.

### Separate real gaps found while investigating

The game calls both `makeObjectsPerformSelector:` and
`makeObjectsPerformSelector:withObject:`. tapHLE implements only the first, and
only on `_tapHLE_NSMutableArray`, so an immutable array cannot receive either.
Neither is the current blocker, but both are genuine reusable gaps.

Rating is unchanged at **★★☆☆☆ (2/5) Starts**.

### The dispatcher holds unretained pointers (2026-07-22, later)

Tracing one `KataCCButtonBehavior` from `alloc` to `dealloc`, recording the
guest `LR` at every reference-count change, gives:

```
retain  -> 2   (host)
retain  -> 3   (guest, LR 0x78b95 -- the dispatcher's send site)
release, was 3 (guest, LR 0x78b95 -- immediately after the send)
release, was 2 (host)
release, was 1 (host)
DEALLOC
```

The dispatcher's retain/release pair brackets the send and nothing else, so it
holds **no persistent reference** — the same contract as iOS, where a list like
this stores unretained pointers and the object is required to unregister
itself. That rules out "the dispatcher forgot to retain" as the defect.

Teardown is not being skipped either. Logging every selector sent to the class
shows `onRemove` and both levels of `dealloc` (the guest override and its
super-call) run on all five buttons. The registration simply outlives the
object.

No object was reported as deallocated with a non-zero refcount, and the counts
above balance, so this is not an over-release. The missing piece is a retain
that iOS performs and tapHLE does not, in whatever **owns** the button — not in
the dispatcher and not in the teardown path.

Note that `-[NSMutableArray removeObject:]` did contain a real bug: it removed
matching indices in ascending order, so each removal shifted the rest down and
every index after the first referred to the wrong element. That was fixed with
a regression test in `673d684e`. It did **not** change this frontier, so do not
assume it was the cause.

The alias registry owner is `KataSVUserBehaviorController`; it performs 53
`mapAlias:toClass:` registrations covering every alias the scene looks up.

A private debugging cache lives outside the checkout at
`RPythonVibecoding/_bm-debug-cache/`: the extracted binary, its armv7
disassembly (addresses match the guest `PC`/`LR` in tapHLE's register dumps), a
menu screenshot, and a PowerShell harness that launches the app, clicks Play,
and prints the failure. None of it may enter the repository.

## Gameplay reached: ★★★☆☆ (3/5) In game (2026-07-23)

Baby Monkey now starts, renders and sustains its gameplay loop. Reproduced from
the clean committed release build of `59d4d97b` against the canonical
hash-verified bytes, in a normal visible window from an isolated `%TEMP%` run
directory: the menu appears, clicking Play loads `bm/GameScene.scene`, the
monkey-on-pig scene renders with its parallax hills, trees, grass and HUD, the
world scrolls, the tutorial ("Tap [monkey] to grab the [banana]") appears on its
own timer, collectible bananas spawn, an in-game tap on the monkey button is
accepted, and the process is still running and animating after about a minute.
No panic, no forced exit before the harness stopped it.

### The 2026-07-22 root cause was right about "use-after-free", wrong about where

That section concluded the defect was "a retain that iOS performs and tapHLE
does not, in whatever **owns** the button". It is not. Static disassembly of the
app settles what the "touch dispatcher" actually is:

- `-[KataEntityBase addObserver:selector:name:]` is a wrapper over
  `[[NSNotificationCenter defaultCenter] addObserver:… selector:… name:…
  object:self]`.
- `-[KataEntityBase dispatch:payload:]` is a wrapper over
  `postNotificationName:object:userInfo:`. Guest `LR 0x78b95`, recorded in that
  section as "the dispatcher's send site", is the return address inside it.
- `-[KataCCButtonBehavior onAwake]` registers `onTouchesBegan:` that way, and
  `onAsleep` removes it with `removeObserver:name:object:`.

So the retain/release pair that bracketed the send, and that the note read as
proof the dispatcher holds no reference, was tapHLE's own `retain`/`release`
around the delivery in `postNotification:` — not guest code at all. The engine's
teardown was correct the whole time.

The real defect was in tapHLE. `postNotification:` copied the matching observer
lists and then iterated the copy blindly. Tapping Play delivers to the Play
button first; its handler tears down the menu scene, which puts all five menu
buttons to sleep — each unregistering itself — and frees them. The stale copy
then sent `onTouchesBegan:` to the freed buttons. That is exactly why four runs
of one build reported four different receiver classes at the same point. Fixed
in `165c873d`: every registration carries an identity, and a copied entry is
re-checked against the live list before its observer is messaged. Real
`NSNotificationCenter` makes the same guarantee, which is what makes the
unretained-observer contract usable at all.

### The rest of the gameplay load

With the touch crash gone, the scene load ran to completion through three more
missing Foundation behaviours, each a general gap rather than anything specific
to this game:

1. `-[NSObject valueForKey:]` did not exist (`2f9c07b5`). The scene load needs
   it in `-[KataCCPrefabController applyInstance:toPrefab:]`, which reads each
   overridden property off a behavior before writing the instance's value.
2. `setValue:forKey:` asserted the value was non-nil (`6a0e77aa`). The same
   round-trip writes back the nil an unset object property reads as. Non-object
   targets now go to `setNilValueForKey:` as documented.
3. `NSMutableSet` had no `setSet:` or `intersectSet:` (`59d4d97b`). Every
   in-game tap needs them: `-[KataCCButtonBehavior getValidTouches:]` keeps a
   `currentTouches` set and reconciles it against each event.

### Known limitations at this rating

- **Some sounds fail to load, but audio works.** Do not repeat the earlier
  claim that the game is silent — it is not, and it was never checked before
  being written down. The maintainer hears audio, and an OpenAL Soft wave
  capture of tapHLE's mixed output agrees: continuous signal across a 48 s run,
  mean −30.4 dB and max −16.5 dB in the menu window and mean −30.3 dB / max
  −16.5 dB in the gameplay window, with the only silence being the final 1.6 s
  after `WM_CLOSE`. What is true is narrower: 30 `AudioFileOpenURL()` calls
  fail with `FileReadError` on bundle-relative `audio/<name>.wav` paths, so
  that loader is not finding its files (they live under `<bundle>/bm/audio/`).
  Which individual sounds are consequently missing has not been established,
  and the music evidently arrives by another route. Establish that before
  treating the path fix as an audio fix.
- Chartboost and Flurry are faked, and `NSURLConnection` is a TODO stub, so the
  ad/analytics paths do nothing. This does not affect gameplay.
- Only the first level segment and the tutorial were observed. Scoring, death,
  game-over, pause and resume are untested. Four and five stars need human
  testing anyway.
- The in-game tap was verified as "accepted without crashing and the loop
  continues". Whether the monkey actually grabs a banana is a gameplay-outcome
  claim that needs a human playtest.

### Next discriminator

First find out what the 30 failing `AudioFileOpenURL()` loads actually cost,
since audio is audible without them. Identify the route the working audio takes,
then fix bundle-relative resource lookup so `audio/<name>.wav` resolves against
`<bundle>/bm/` and measure the difference with the OpenAL Soft wave-writer
capture described in the playbook rather than assuming one. After that, play
through to a death and a game-over screen to find the next frontier.

## Blank window, and the general presentation defect (2026-08-03)

Baby Monkey renders nothing: the window is a flat slate rectangle from
launch. The three-star result above is therefore currently invisible to
anyone who runs the game.

**This is not caused by the 2026-08-03 session's work.** A build of
`400d2b7d` — trunk as it stood before that session — shows the identical
blank window with the identical bytes. Whatever broke it landed somewhere in
the 437 commits between `59d4d97b` (where the menu was last seen) and
`400d2b7d`, and bisecting that range is the outstanding job.

### The frame is fine; presentation loses it

Capture the guest renderbuffer with `TAPHLE_FRAME_CAPTURE_REQUEST` /
`TAPHLE_FRAME_CAPTURE_OUTPUT` and screenshot the window at the same moment.
The renderbuffer holds the complete menu — Play button, monkey, pig, trees,
mountains, the four bottom-bar buttons — and the window holds none of it.

The drawable is 480x320, already landscape. tapHLE nevertheless applies its
portrait-to-landscape presentation rotation, which samples an
already-upright frame through a rotated texture matrix with `CLAMP_TO_EDGE`,
so the window receives edge-clamped bands rather than the image.
`--landscape-native` disables that rotation and both the menu and gameplay
appear. It is now the app's default option.

### Why this is a tapHLE defect, not two app quirks

Cubed Rally Redline needs the same option for the same measured reason. Two
unrelated games, one Unity and one a custom engine, both render
landscape-native and both are destroyed by the same rotation. Any other
landscape game whose drawable is landscape-shaped is affected too.

A general fix is possible: when the app's EAGL drawable is landscape-shaped
while tapHLE has the device rotated to landscape, the app is drawing
landscape-native and the presentation rotation must not be applied. A
correctly-behaving landscape app renders into a 320x480 portrait drawable and
still needs it, so the drawable's aspect is the discriminator.

**Do not implement that as a presentation-only change.** `transform_input_coords`
in `src/window.rs` rotates incoming touches by the inverse of the same matrix.
Changing presentation alone would leave every tap landing in the wrong place.
The fix has to put the window into the landscape-native regime — the same
state `--landscape-native` produces — so that presentation, the viewport, and
input all agree. That is why this session shipped the per-app option instead:
it is the same end state, reached safely, without changing behaviour for
every other landscape game on the way.

### Controller mapping

Read off the rendered screen, not guessed:

| Control | Screen target | What it does |
| --- | --- | --- |
| A | (43, 274) | monkey button, bottom left — jumps the monkey off the pig |
| B | (439, 274) | pig button, bottom right |
| Start | (22, 22) | pause, top left |

Tilt already reaches the game through the analog stick, and the app's
`Info.plist` requires the `accelerometer` capability. No `--y-tilt-offset` is
asserted: the game's neutral holding angle is not something a screenshot can
establish, and picking a number without a human holding a controller would be
a guess recorded as a default. Establish it with a playtest.

All three targets were exercised with real foreground input. Play at
(240, 120) starts a run, the monkey button jumps, and Start's target at
(22, 22) — checked separately on 2026-08-04, since it had been left
unconfirmed — brings up the Paused overlay with Resume / Main Menu / Replay.

Only the *targets* are confirmed this way; a tap is not a controller. Whether
the pad's A/B/Start buttons actually reach those coordinates needs a playtest
with hardware, as does the tilt neutral above.
