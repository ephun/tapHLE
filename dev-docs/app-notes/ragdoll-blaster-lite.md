# Ragdoll Blaster Lite compatibility work note

- Branch: work done directly on `trunk` (reusable fixes only so far).
- Canonical artifact:
  <https://archive.org/download/ragdoll-lite-v-1.4-clutch-1.3/Ragdoll%20Lite-v1.4-(Clutch-1.3).ipa>
  (identifier `ragdoll-lite-v-1.4-clutch-1.3`), `source: original`;
  size 5,789,885 bytes.
- Hashes:
  - MD5: `4977342b062e26f6c3fe9344434ac702`
  - SHA-1: `8dac50a93c1eefdc638fdb15f83bc2113ba048fb`
  - SHA-256: `a9c888b0aacbf3ecb9fa9e0d7a777c6e162ff796d2fc9ffcfa6afcb7aeb3b678`
- Embedded identity (`tapHLE --info`): display name `Ragdoll Lite`, bundle
  `com.backflipstudios.RagdollBlasterLite`, version `1.4`, minimum OS `2.2.1`,
  iPhone. Note the bundle identifier says **RagdollBlasterLite** while the
  display name is just "Ragdoll Lite"; the Archive item is named for the
  latter.
- tapHLEdb: App 31, version 32, report 51 (2026-07-27, tapHLE `28eb65fe`,
  ★★☆☆☆). The app row was created by that submission.

## Current state: no frame

## Cleared

1. `-[NSBundle initWithPath:]` did not exist. Adding it also needed
   `+[NSBundle allocWithZone:]`, because `+alloc` on NSBundle fell through to
   NSObject and produced an object with no `NSBundleHostObject` behind it —
   `-initWithPath:` would have panicked on the first borrow. It returns nil for
   a path that does not exist, which Apple documents and which is how an app
   asks "is there a bundle here?".

## UITableViewController is implemented; the frontier has moved

`UITableView`, `UITableViewCell` and `UITableViewController` now exist on
`trunk`, with `NSIndexPath` filled in underneath them. This app was the first
test for that work, as planned — it hits the class during startup, which made
the edit-run cycle minutes shorter than Tap Tap Revenge 3's four-tap path.

## Highest milestone: 2-star (Starts / Menu), tapHLE `28eb65fe`

It draws. The **Ragdoll Blaster 2 promo screen** renders in full — the artwork,
the "Now Available!" banner and the Get it Now / No Thanks buttons — and the
No Thanks button is accepted. Filed as report 51.

Past it the screen shows only the graph-paper level background and **stops
changing**: repeated captures are byte-identical by SHA-256 while the process
stays alive and the run loop keeps turning. So the app gets past the promo, puts
up the background for whatever comes next, and then does not draw it.

### What it took

Nine general gaps, all on `trunk`, none specific to this app:

1. `-[NSBundle initWithPath:]`, which also needed `+[NSBundle allocWithZone:]`.
2. `UITableView`, `UITableViewCell`, `UITableViewController`.
3. `NSIndexPath`, which was an empty stub.
4. `UILocalizedIndexedCollation`.
5. `UIBarItem` and `UIBarButtonItem`, plus a real `UINavigationItem` and
   `-[UIViewController navigationItem]`.
6. `_setjmp`/`_longjmp` — the no-signal-mask variants; `setjmp` itself already
   existed, which is why this was a small job rather than a large one.
7. `CGContextGetTextPosition`/`SetTextPosition`.
8. XML property list output.
9. `-popToRootViewControllerAnimated:` and `-popToViewController:animated:`.

### Current frontier: the level screen draws only its background

Nothing is logged when it stalls — no missing selector, no unimplemented class,
no fault. The app is alive and idle.

Two candidates worth separating before writing any code:

- **It is waiting for something.** The last thing in the log before the stall is
  a faked `FlurryAPI logEvent:withParameters:`. This app would be the *fourth*
  on the target list to stall or die inside an analytics SDK (see
  `jellycar.md`), so check whether the level screen's setup runs on a
  completion callback that never arrives.
- **It drew nothing because it has nothing to draw.** The screen after the
  promo is reached through the `UITableView` added for this app. A table whose
  data source returns zero rows would look exactly like this. That is testable
  in one run by logging the row count in `reload`.

The second is cheap and rules out the newest code, so it was done first — and
it **rules the table view out**. With the table view's reload logging enabled
(`TAPHLE_LOG_MODULES=tapHLE::frameworks::uikit::ui_view::ui_scroll_view::ui_table_view`),
a full run to the stall produces **no reload lines at all**: no table view is
ever populated, so the level screen is not one. The new code is not the cause.

### Correction: it is not stalled, it is drawing an empty screen

The word "stall" above was wrong, and the analytics hypothesis with it. Tracing
every selector across the tap and the following eighteen seconds produced
**49,499 further messages**, and the tail is an ordinary render loop:
`EAGLContext presentRenderbuffer:` every frame, timers firing, the window
compositing.

So the app is **not blocked and not waiting**. It is running normally and
drawing the same frame each time, which is what a static screen looks like from
the outside. Byte-identical captures were consistent with a hang and are equally
consistent with this; the trace is what distinguishes them, and it should have
been run before a cause was proposed.

Both earlier candidates are therefore dead: the table view was already ruled
out, and an app waiting on an analytics callback would not be presenting frames.

### The actual question

The promo screen drew correctly through the same EAGL path, and the graph-paper
background draws now, so the drawable and the compositor are fine. What is
missing is the level screen's *content* on top of it.

The next step is to find out what that content is and why it produces nothing:
whether the level data or its textures failed to load, or whether the geometry
is being drawn somewhere off screen. Start from the GL calls between two
consecutive `presentRenderbuffer:` calls — if the frame contains only a
background blit, the content was never submitted, and the problem is upstream in
loading rather than in rendering.
