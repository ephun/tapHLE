# Tap Tap Revenge 3 compatibility work note

- Branch: `compat/tap-tap-revenge-3`. Reusable fixes graduated to `trunk`.
- Canonical artifact:
  <https://archive.org/download/ios3-6-ipas/com.tapulous.taptaprevengeIII.ipa>
  (identifier `ios3-6-ipas`), `source: original`; size 14,448,768 bytes.
- Hashes:
  - MD5: `60d59a05fd0f1134ffe0268d8cea4988`
  - SHA-1: `0722dec3bacba8ff3f2c6351681e54ca43969b7f`
  - SHA-256: `5c37763200da8dba06346164e8f104b7fb82b1de883d4f58d305109dbc978cec`
- tapHLEdb: App 15, version 31, report 48 (2026-07-27, tapHLE `b953ef9c`,
  ★★☆☆☆).

## Highest milestone: 2-star (Starts / Menu), tapHLE `b953ef9c`

It launches. The main menu renders in full: the title, the tutorial banner, the
avatar panel, the "You are offline" notice, the Online / 1 Player / 2 Player
mode wheel, and the Play button. Verified on a clean committed build (window
title `Tap Tap (tapHLE b953ef9c)`, no `-dirty`).

This is the same Tapulous codebase as Tap Tap Revenge 2, so everything cleared
for that app is a prerequisite and is already on `trunk`. TTR3 needed nine more,
all general and all on `trunk`:

1. `-[NSTimer initWithFireDate:interval:target:selector:userInfo:repeats:]`,
   the designated initialiser. Adding it meant raising the host method arity
   ceiling: `impl_HostIMP!` stopped at five method arguments and this selector
   has six. `CallFromGuest` already went to eleven, so that was an oversight
   rather than a limit.
2. An already-deallocated object found in a draining autorelease pool now warns
   and skips instead of aborting.
3. `-[UIView autoresizingMask]` and `-autoresizesSubviews` — setters that
   discarded their argument, with no getters at all.
4. `-[UIWebView initWithCoder:]`, which was `todo!()` and took the whole nib
   down with it.
5. `-setNeedsLayout` and `-layoutIfNeeded`, deferred to the run loop.
6. `-[NSScanner scanInteger:]`.
7. `-[NSArray makeObjectsPerformSelector:]` and the `withObject:` form.
8. CGPath, plus path fill and stroke on CGContext — neither existed at all.
9. Re-parenting a guest subclass of a Foundation cluster class onto tapHLE's
   concrete implementation.

## UITableView is implemented; the frontier has moved

`UITableView`, `UITableViewCell` and `UITableViewController` now exist on
`trunk`, along with `NSIndexPath` (which was an empty stub) and
`UILocalizedIndexedCollation`. Tapping Play gets through all of them.

The table view **builds every row up front instead of recycling cells**. That
is a deliberate trade documented in its own module: the protocols, selection
and scrolling work without visible-rectangle bookkeeping, at the cost of being
wrong for a table with thousands of rows. This app's track list has tens.

## Current frontier: a guest MemoryError after the track list loads

```text
Error during CPU execution: MemoryError
```

No missing selector, no unimplemented class — the guest itself faults. That is
a different class of problem from everything cleared so far and needs the
register dump and a disassembly around the faulting PC, per the debugging
playbook, rather than another round of "add the method it asked for".

Worth checking first, because it is cheap and this app has form for it: three
apps on this target list die inside bundled analytics SDKs (see
`jellycar.md`). Look at what runs between the table appearing and the fault
before assuming the table view is at fault.

Do **not** assume the new table view is the cause without evidence. It is the
newest code in the path and therefore the obvious suspect, which is exactly why
it deserves a measurement rather than a hunch — the eight-app regression sweep
and Ragdoll Blaster Lite both exercise it without faulting.

## 2026-08-15: the song list is populated and a song loads

Still 2 stars — no gameplay yet — but the frontier moved a long way, and six
general gaps were closed to do it. Every one of them was a tapHLE gap rather
than something about this app.

The `MemoryError` recorded above is **gone**, and so is a regression that had
appeared since: the app could not even reach its main menu, aborting inside
`-[NSInvocation invoke]`, which asserted every argument slot had been set. It
does not need to be, and Apple's does not require it.

Closed on `trunk`, in this order, each one the next thing that stopped the app:

1. `-[NSInvocation invoke]` passes an unset argument as zero.
2. The `kCATransition*` subtype names and `kCATransition` are exported. The
   fault was the classic unbound non-lazy slot: `ldr r2,[r2]` at `0x60e28`
   reading the null slot at `0x18135c`.
3. `UITableView` loads its rows on its first layout, not only in `-reloadData`.
   **This was the empty song list**: the app sets a data source in a nib and
   never calls `-reloadData`, because on a device it does not have to.
4. `CFSet` — the whole type was missing.
5. `+[NSString stringWithContentsOfFile:usedEncoding:error:]`, which is how the
   app loads the Lua configuration in `game_defaults.cfg` and `theme.cfg`.
6. `-[NSString compare:]` with a nil argument no longer aborts, and the
   clipping and truncating line break modes no longer abort.

### The route

Window is 320x480 portrait, ~35 s to the menu.

1. Menu: Play at client `(159, 390)`.
2. Song list (Career, with Easy/Medium/Hard/Extreme tabs across the top): the
   first song row is at about `(160, 120)`, the second `(160, 200)`.
3. That reaches the **loading screen**: the song's artwork and title, a
   "Loading" banner and a gameplay tip.

### Current frontier: the loading screen never finishes

The app is alive there — it is not a crash and not a hang in the emulator — but
after 50 s it is still loading, and the tail of the log is only
`pthread_testcancel()` and `Attempting to grow heap.` repeating. Something is
looping and allocating without bound.

What is known about that point:

- The song file opens (`AudioFileOpenURL()` succeeds on the track's `audio.m4a`,
  and tapHLE does have AAC/MP4 support), an output queue is created, its volume
  is set, and the queue is **Reset** — and then nothing. No buffers are
  allocated and it is never started.
- `AudioQueueAddPropertyListener(..., 'aqsr', ...)` is a TODO, so if the app is
  waiting to be told a queue property changed, it will wait forever. That is a
  hypothesis, not a finding.
- The app's own log says `Warning: Receiver method signature is nil, for
  receiver TTRLuaCallGameEntity` once, which is its Lua-to-Objective-C bridge
  failing to build an invocation for something. Worth identifying: log the nil
  return in `-methodSignatureForSelector:` and see which selector it wants.

Next discriminator: find which of those two is the loop. The cheapest cut is to
log the nil case in `-[NSObject methodSignatureForSelector:]` and, separately,
to see whether the allocating loop stops if the audio queue is left out — but
do not start by implementing the property listener, because nothing yet shows
the app is waiting on it.

### A rendering bug worth its own branch

Text that tapHLE draws itself comes out **mirrored left-to-right** on this app's
screens: the song titles, the tip text, the nav bar titles and the difficulty
tab labels all read backwards, while text baked into the app's own images is
correct. Some labels ("Loading", "Tip") are correct, so it is not every path.
This is general, not specific to this app, and is not investigated here.

## A boundary that went unreported, recorded here because it cannot be filed

On `8ec4049e` this app was **1 star**: it aborted inside `-[NSInvocation
invoke]` before drawing anything, against the 2 stars report 48 records. That
was measured directly, with a log, and **no report was filed at the time**. The
fix that restored it to 2 stars went unfiled too.

Neither can be recovered now. A report asserts that an artifact was run at a
revision and rated *then*, and composing one afterwards from a note or a later
rerun cannot honestly assert it — so the remedy the guide gives is this entry
plus submitting the rating the app holds now. It holds 2 stars, which report 48
already records for this app, so submitting again would be moderation noise and
nothing is filed.

What that costs: the database has no evidence that `8ec4049e` was the revision
where this app could not start, which is exactly what a later regression hunt
would want. File the boundary when it is crossed.
