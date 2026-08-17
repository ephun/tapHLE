# The Jim and Frank Mysteries HD compatibility work note

- Branch: `compat/jim-and-frank-hd`. Reusable fixes graduated to `trunk`.
- Canonical artifact:
  <https://archive.org/details/ios-ipa-com.chillingo.thejimandfrankmysterieshd>,
  file `J & F HD (v1.1) [Cracked].ipa`, `source: original`; size 398,940,007
  bytes. The item lists a second name for the same bytes (the full
  "The Jim and Frank Mysteries ... HD (v1.1) [Decrypted].ipa"); the short name
  was chosen only because it is ASCII-safe on Windows. Same MD5.
- Hashes:
  - MD5: `8c20da73edbbd7d7d2114b1a8ff2ac6a`
  - SHA-1: `543f59387288315eacc46620116df06a5720d057`
- Embedded identity: bundle `com.chillingo.thejimandfrankmysterieshd`,
  version `1.1`.
- tapHLEdb: App 20, version 20, report 28 (2026-07-26, tapHLE `4e246384`,
  ★☆☆☆☆).

## Clickmap

`dev-docs/clickmaps/jim-and-frank-hd.json` replays launch through profile
creation, the introduction cutscene and chapter one to the second room. Run it
with `dev-scripts/clickmap.ps1` before working out a route by hand. Pass
`-HasSaveState` when a profile already exists: the profile-creation steps are
skipped and Play lands on Continue instead.

## 2026-08-04: the menu is on screen. Two stars.

**Everything below this section is history.** It describes a one-star app whose
window died before anything was presented, and that is no longer what happens.
Read it for the reasoning, not for the current state.

Measured on `1c22d176`, a clean worktree, with an OS-level `PrintWindow`
screenshot — the same method that overturned the earlier two-star claim, and the
only one this app's history says to trust:

- The Chillingo launch image comes up **upright, landscape, correctly shaped**.
- By twelve seconds the **full main menu** is on screen: the illustrated scene,
  the title *The Jim & Frank Mysteries — The Blood River Files*, and all five
  signposts (Play Game, Select Profile, Options, Extras, About).
- It is **stable**: a capture at thirty seconds is identical to the one at
  twelve. Not a transient frame.

Identity, read from `--info` and not from the filename: `J & F HD`,
`com.chillingo.thejimandfrankmysterieshd`, version `1.1`, minimum OS `3.2`,
device family iPad.

### What fixed it, and why nothing here had to be redone

Three changes that landed on `trunk` for other reasons, none of them this app:

1. `2cf13f66` — `UIApplicationMain` now owns the main nib's top-level objects
   and makes its window key. This is exactly the retain that the note below
   describes as "correct on its own terms" but deliberately withheld because,
   alone, it turned a rendering app grey.
2. `0f9d5a16` — the compositor's presented texture wraps again.
3. `c0cdbd1c` — an app launches in the orientation its `UIInterfaceOrientation`
   asks for. This app asks for `UIInterfaceOrientationLandscapeRight`.

Together they resolve the conflict the note below could only describe: the
window survives *and* the GL content reaches the screen. The `CAEAGLLayer` is
now in the composited tree — traced with `TAPHLE_LOG_MODULES`, it composites
every frame at 768x1024 with `eagl_pixels=true` — and `MOUNT [UIWindow
addSubview:EAGLView]` appears, which is the mount the last section said never
happened.

So the closing question of the previous section — "what is supposed to put the
EAGLView on screen" — is answered: the app always did it, and could not while
the window it queried for had already been deallocated.

### Reported

tapHLEdb report 66 (2026-08-05, tapHLE `a92bdd40`, ★★☆☆☆), against the existing
app 20 and version 20. Re-verified on that revision before submitting rather
than filed from the earlier `1c22d176` run, so the report cites what was
actually tested.

### 2026-08-06: three stars is blocked on the Crystal SDK, not on tapHLE

Play Game does nothing. Established, in order, so the next person does not
repeat it:

1. Touches arrive correctly. `ui_touch` shows `touchesBegan:`/`touchesEnded:`
   delivered at the right guest coordinate.
2. They go to `CCSkinnedView` — Chillingo's Crystal skinning view — which is
   mounted **last** on the window and so is frontmost, with a full-screen frame.
3. That class **does** implement `touchesBegan:`, read straight out of the
   binary's `__objc_classlist`. Its own handler runs and passes the touch up the
   responder chain, so it has decided the tap is not on its content. The
   cocos2d menu behind it never sees it.
4. tapHLE is not at fault in the obvious places: hit-test order is front-to-back
   as UIKit's is, `UIResponder` forwards unhandled touches correctly, and
   `superlayer_to_layer_transform` does include the affine transform.
5. `CCSkinnedView` sizes itself with `updateFrameForContent`/`sizeThatFits:`,
   is created `0x0`, and ends up 768x1024 — and **the app does that itself**.
   A message trace shows the app calling
   `initWithUIElement:skinDescription:frame:formModel:shouldCache:` ->
   `initWithFrame:` -> `setFrame:`. tapHLE does not resize it: the autorotation
   path that could have is skipped, because the controller answers `false` to
   `shouldAutorotateToInterfaceOrientation:`.

6. **The "missing" skin images are not missing. This was my own false lead and
   it is recorded so nobody follows it again.** The warning
   `couldn't read image file at .../Images/Images/MainMenu/start.png` is real,
   but it is the app's *first* of two attempts. With
   `TAPHLE_LOG_MODULES=tapHLE::frameworks::foundation::ns_string` the very next
   line shows it retrying `Images/MainMenu/start.png` and succeeding. Same for
   `MainMenu_Crystal_btn12x203.png`. tapHLE logs the failed attempt at warning
   level and the successful retry only under module logging, which is what made
   it look like a failure. The skin loads.

7. Its handler is not failing to read the touch either. Disassembling
   `-[CCSkinnedView touchesBegan:withEvent:]` (imp `0xe0509`, Thumb) shows it
   opens with `countByEnumeratingWithState:objects:count:` over the touches set
   and branches to the end when the count is zero — which would have explained
   everything and would have been tapHLE's fault. It is not what happens: a
   message trace shows the enumeration running on `_tapHLE_NSMutableSet` and
   going on through `allObjects`, `count` and `objectAtIndex:`. The loop body
   runs with the touch in hand, and the handler decides not to act on it.

8. tapHLE's resource lookup is correct. `resourcePath` and `bundlePath` both
   return the bundle path, as iOS does for an app bundle, and all eleven
   `pathForResource:` calls the app makes are well-formed.

9. `userInteractionEnabled` is not the explanation either, which is worth
   stating because it is the obvious last candidate: tapHLE's `hitTest:` does
   skip a subview whose value is false, the property is implemented and stored
   correctly on `UIViewHostObject`, and a message trace shows the app never
   sends `setUserInteractionEnabled:` to anything. The view is interactive
   because UIKit's default is interactive, which is also true on a device.

So the emulator side is exhausted: touch coordinates, hit-test order, responder
forwarding, layer transforms, resource paths, the touches set, `NSSet` fast
enumeration, the handler's execution, and the view's geometry have each been
measured and are each correct. What remains is that `CCSkinnedView` legitimately
covers the screen, legitimately receives the tap, and its own logic declines it —
so the cocos2d signposts behind it never see it. Why that differs on a real
device is a question about the Crystal SDK's internal state, not about tapHLE.

**Do not resume by assuming the touch coordinates are wrong.** They were tested
and they are right. A mirrored click appears to "work" only because the mirror
of Play Game is where the Crystal gem icon actually sits, and that icon is a
real button — it opens Crystal's networking path, which then asserts
(`CCServerDataHTTP: no GET connection constructed`) and passes nil to
`to_rust_string`.

So the remaining work is Crystal SDK behaviour, not an identified tapHLE gap,
and it needs someone who can watch the overlay respond to real taps. `UITableView`
section metrics were implemented along the way because Crystal's list needed
them, and that is on `trunk`.

### What is not yet known

Nobody has pressed a button. Two stars is "reaches a stable screen"; three
requires a gameplay loop that starts and persists, which needs a real tap on
Play Game and is the obvious next step. The menu geometry recorded below is
stale — it was measured in a 768x1024 window, and the window is now landscape.

Still missing statically, from `dev-scripts/demand.py app`: the AddressBook
framework entire, `NSHTTPCookie`, `NSHTTPURLResponse`, `NSURLCredential`,
`SKPayment`, `UISwipeGestureRecognizer`, and 35 symbols. None of them stopped it
reaching the menu.

## CORRECTED: 1 star. The menu was never on screen

**Report 50 said two stars and was wrong.** Superseded by report 52 (1 star) on
`425c7138`.

The "full main menu" recorded here was read out of tapHLE's own frame capture.
For this app that capture is the **renderbuffer the app submitted**, not the
screen — `capture_renderbuffer` runs inside `-presentRenderbuffer:` before the
fast/slow branch, before host rotation and before composition, and its own doc
comment says so.

An OS-level `PrintWindow` screenshot of the tapHLE window, which is outside
tapHLE's GL code entirely, shows what is actually there: **the Chillingo splash
logo, rotated 90 degrees, frozen**. It never updates; sampling gives 140 distinct
colours, all of them the splash.

So the app draws its menu correctly into its own buffer, and that buffer never
reaches the screen. One star: it does not reach usable content.

### Why nothing reaches the screen

Only three call sites present anything to the window: the splash path in
`window.rs`, `composition.rs`, and the EAGL fast path in `eagl.rs`. With the
nib's window deallocated the window list is empty, so `recomposite_if_necessary`
returns early *and* `find_fullscreen_eagl_layer` returns nil. **No `swap_window`
runs after the splash at all.** The slow path still reads the renderbuffer back
into `presented_pixels`, for a layer tree nobody composites.

That also disposes of two things recorded earlier as defects. The 90 degree
rotation in captures proved nothing, because the capture is taken before host
rotation — a landscape app's renderbuffer is necessarily sideways in a capture
whether or not the screen would be upright. And `--landscape-native` leaving
captures byte-identical is expected, because that option only affects
`present_frame`, which `capture_renderbuffer` bypasses.

### With the retain applied, the grey is real

Re-measured with an OS screenshot rather than a capture: uniform `#999999` over
the whole window, 12 distinct sampled colours. It is **not** the `0x7f` fill
tapHLE uses to make a dead capture obvious, so it is genuinely being drawn.
Retaining the nib objects trades a frozen splash for a grey screen — both one
star — so the retain is still not committed, but now for a measured reason.

`NSDecimalNumber` is what unblocked it: the class did not exist and startup
aborted on `+decimalNumberWithString:`. It is on `trunk`, backed by a `double`
with the precision limitation stated in its own doc comment.

## Two separate problems block three stars

### 1. The scene is drawn rotated 90 degrees

Everything renders, sideways. `--landscape-native` changes the window from
768x1024 to 1024x768 but **does not** change the rendering — the captured
renderbuffer is byte-identical with and without it (1,493,279 bytes both ways).
So whatever that option does, it does not reach this app's path, and the
sideways output is not simply "the option was not passed".

### 2. Touches are discarded: the app never creates a UIWindow

```text
Couldn't find a window for touch at CGPoint { x: 110.0, y: 170.0 }, discarding
```

`ui_touch` walks `ui_view::ui_window::windows` and finds none containing the
point. Tracing `makeKeyAndVisible`, `setFrame:`, `initWithFrame:` and
`pointInside:withEvent:` across a whole startup produced **no UIWindow messages
at all**, so the list is not merely mis-positioned — nothing was ever added to
it.

That is the more interesting of the two, and it should be settled before the
rotation: an app that renders through EAGL without a UIWindow still receives
touches on a device, so tapHLE is missing whatever creates or registers that
window. Find what the app *does* create in place of one — trace class
allocation during startup and look for its own window or view-controller class
— before assuming this is a UIKit gap.

Menu geometry, if it becomes useful: in the 768x1024 capture, "Play Game" sits
at about `(110, 170)`, with the other planks to its right along the top edge.
Both that point and its landscape rotation `(853, 110)` were discarded.

## 2026-07-27: the touch problem narrowed to the main nib

The earlier note said the app "never creates a UIWindow" and suggested finding
what it creates instead. That framing was wrong in an instructive way: **the app
does not create a window because it does not have to.**

Established, in order:

1. The binary references `UIWindow` and `makeKeyAndVisible`, so it is not
   avoiding UIKit windows on principle.
2. Tracing shows it calls `[UIApplication keyWindow]` and
   `[UIApplication windows]` and **never** `makeKeyAndVisible`. It *queries* for
   a window rather than making one — which is correct iOS code for an app whose
   window comes from its main nib.
3. Its `Info.plist` has `NSMainNibFile = MainWindow` and the bundle ships
   `MainWindow.nib`. There is no `NSMainNibFile~ipad`, and
   `Bundle::main_nib_filename` falls back to the plain key correctly, so the
   right name is resolved.
4. `UIApplicationMain` does load the main nib, and nib machinery demonstrably
   runs — the log carries a `UIProxyObject` line from that load.
5. `-[UIWindow initWithFrame:]` **and** `-initWithCoder:` both register the new
   window in `ui_view.ui_window.windows`, so a window created by either route
   would be found by hit-testing.

### Answered: the window is created, then deallocated

That prediction was wrong too, and measuring it took one run. The nib logging
added to `ui_nib` reports:

```text
Nib instantiated 4 top-level object(s) (of 6 decoded):
  ["OdysseyAppDelegate", "UIWindow", "OdysseyAppController", "EAGLView"]
```

The window **is** produced. And the touch failure now reports the candidates:

```text
Couldn't find a window for touch at ..., discarding. Windows: []
```

Created and registered, then gone. Nothing retains the main nib's top-level
objects, so they are autoreleased and die at the first pool drain — taking the
window's registration with them via `-[UIWindow dealloc]`. UIKit's contract is
the opposite: a nib's top-level objects come back at +1 and the loader owns
them.

### Why the obvious fix is not committed

Retaining them in `UIApplicationMain` **does** fix touches — the discard message
disappears entirely. It also turns the screen grey: the nib's `UIWindow`, now
alive, composites over the `EAGLView` the app actually draws through, and the
window is empty.

So the app trades "renders correctly, ignores every tap" for "accepts taps,
draws nothing". That is not an improvement and it is not committed. The two
diagnostics that found all this are, since they are useful regardless.

### What the real fix has to reconcile

Traced with the retain applied as an experiment (not committed), the app *does*
build its hierarchy: `[UIWindow addSubview:]` is sent twice, `[EAGLView
addSubview:]` once, and both the window and the `CAEAGLLayer` get `setHidden:`.
So window-contains-view is established, and the window is not empty.

Two things came out of that run. **The first turned out not to be a second bug
at all**, which is worth spelling out because it changes the shape of the work:

**1. The nil `addSubview:` receiver *is* the dead window.** Tracing everything
and reading the messages either side of it gives the whole story in eight lines:

```text
[UIWindow dealloc]                                  <- nib objects unretained
[OdysseyAppDelegate applicationDidFinishLaunching:]
[UIApplication keyWindow]                           <- returns nil, it just died
[OdysseyAppController view]
[nil ((null)) addSubview:]                          <- keyWindow's nil result
```

The app asks `UIApplication` for the key window and adds its view to whatever
comes back. The window has already been deallocated, so it adds to nil. This is
not an independent missing reference — it is the same lifetime bug one step
later, and fixing the retain fixes both.

**2. The grey is a compositing conflict, not an empty window.** tapHLE presents
a fullscreen `CAEAGLLayer` directly (`find_fullscreen_eagl_layer`) *and*
composites the Core Animation tree. With the window dead, only the GL path drew
and the app looked correct. With the window alive, the window's layer draws over
the GL content. Retaining the nib objects is right on its own terms — UIKit's
ownership contract says so — but it cannot land until that conflict is resolved,
or it turns a rendering app into a grey one.

So there is **one** blocker, not two: retain the nib's top-level objects, and
resolve the compositing overlap that retaining exposes. The retain is correct on
its own terms and is deliberately absent from `trunk` only because, alone, it
turns a rendering app into a grey one.

### The compositing question, now measured

`find_fullscreen_eagl_layer` logs why it declines, and with the retain applied it
says the same thing every frame:

```text
Not a fullscreen EAGL layer: bounds 768x1024 (screen 768x1024),
  anchor (0.5,0.5), position (384,512), hidden false,
  opacity 0.5, identity transform false
```

Bounds, origin, anchor, position and visibility are all exactly right. **Two
tests fail: `opacity 0.5`, and a non-identity affine transform.**

The transform is the 90° rotation this app uses — it is a landscape game, which
is also why it renders sideways under the direct path. And that rejection is
already anticipated in the function itself:

```rust
// TODO: support affine transforms that result in a full-screen layer
//       (typical example is 90° rotation).
```

So the remaining work is that TODO, plus deciding what a half-opaque fullscreen
layer should mean. Neither is app-specific: any landscape app that gets its
window from a nib will land here, which is a large share of iPad titles.

### One more hypothesis, also wrong: composition does apply the rotation

It would be natural to conclude from the rejection above that the sideways
rendering is composition ignoring the layer's transform. It is not.
`CALayerHostObject::superlayer_to_layer_transform()` concatenates
`self.affine_transform`, and `composition.rs` multiplies that into its
cumulative transform for every layer. Rotation is handled.

So the sideways image is the app drawing landscape content, which is what it
does on a device held sideways — a window-orientation matter rather than a
rendering bug, and consistent with the earlier observation that
`--landscape-native` changes the window size while leaving the captured
renderbuffer byte-identical.

**Four hypotheses have now been tested on this app and three were wrong.** Each
was settled by one cheap measurement — a trace, a log line, or reading the
function being blamed. That ratio is the useful thing to carry forward: on this
app, measure first, because reasoning about it has a poor record.

### Summary of the whole chain

One bug and one gap, in order:

1. A nib's top-level objects are not retained, so the window dies. That causes
   both the discarded touches and the nil `addSubview:`.
2. Retaining it exposes the compositing path, which declines direct EAGL
   presentation for a rotated, half-opaque layer and composites a window over
   the GL content instead.

Fixing (1) without (2) trades a rendering app for a grey one, which is why the
retain is not on `trunk`.

This is still expected to be general: any app whose window comes from its main
nib rather than from code has the same lifetime problem.


## 2026-07-27, later: the GL view is never mounted

Four measurements, each from one instrumented run, settle what the grey is and
where the game's rendering goes. All of the instrumentation is on `trunk` and
enabled with `TAPHLE_LOG_MODULES`.

### 1. What composition actually draws

With the window retained, exactly three layers are composited, every frame:

```text
composite <window> CALayer opacity=1   bg=rgba(1,1,1,1) contents=false eagl_pixels=false sublayers=2
composite <a>      CALayer opacity=0.4 bg=rgba(0,0,0,1) contents=false eagl_pixels=false sublayers=0
composite <b>      CALayer opacity=1   bg=rgba(0,0,0,0) contents=false eagl_pixels=false sublayers=0
```

**No `CAEAGLLayer` appears at all** — and the probe sits above the `hidden`
early-return, so it is not being skipped for being hidden. It is simply not in
the tree.

That also explains the grey exactly, with no guesswork: white background at
full opacity, then black at 0.4 over it, gives `1 x (1 - 0.4) = 0.6`, and
`0.6 x 255 = 153 = #999999` — precisely the value the OS screenshot measured.
The screen is the window plus two fade overlays and nothing else.

### 2. Where the views go

```text
MOUNT [EAGLView addSubview:UIImageView]
MOUNT [UIWindow addSubview:UIView]
MOUNT [UIWindow addSubview:CCSkinnedView]
```

The window gets a plain `UIView` and a `CCSkinnedView` (an overlay). **The
EAGLView is never added to the window**, so the compositor never sees it.

### 3. The outlets are fine — this is not a nib bug

```text
OUTLET [UIApplication].delegate       = OdysseyAppDelegate
OUTLET [OdysseyAppController].view    = EAGLView
OUTLET [OdysseyAppDelegate].mViewController = OdysseyAppController
KVC set 'view' on OdysseyAppController -> setView:
```

All three outlets connect, and KVC resolves `view` to the real `-setView:`
accessor rather than silently falling back to an ivar. The nib controller's
view *is* the EAGLView. Static inspection of `MainWindow.nib` agrees: it is a
`NIBArchive` whose only outlet labels are `delegate`, `view` and
`mViewController`, and it contains **no `UISubviews` key at all**, so the
EAGLView is legitimately a top-level object rather than nested in the window.

### 4. There are two view controllers

`-[UIViewController initWithCoder:]` decodes key `UIView`, gets nil for this
nib, and sets the view to nil; the outlet then supplies the EAGLView. But a
*second* controller is created at runtime with its own view, and it is that
second view which reaches the window.

### Where this leaves it

Still one star. The chain is: nib top-level objects unretained -> window dies ->
nothing presented. Retaining fixes the window but reveals that the app's GL view
was never going to be composited anyway, because nothing mounts it.

The next question is the app's own: what is supposed to put the EAGLView on
screen, and which tapHLE gap stops that code running? `CCSkinnedView` says
Cocos2D, so the engine's own view-setup path is the place to look. Do **not**
resume by retaining the nib objects and calling it fixed — that is measured to
produce a grey screen, for the reason computed above.
