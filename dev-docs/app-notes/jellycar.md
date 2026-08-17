# JellyCar (1, 2, 3) compatibility work note

> **JellyCar 2 is filed (report 39). JellyCar 3 is NOT — tapHLEdb 504s on it.**
>
> - **JellyCar 2 on iPad**, `com.disney.jellyCar2foriPad` 1.1 — **3 stars** on
>   tapHLE `d335f7bd`. Submitted: app 30, version 30, report 39.
> - **JellyCar 3**, `com.disney.JellyCar3` 1.2 — **1 star** on tapHLE
>   `f8a085d8`. Frontier: past resource enumeration, guest MemoryError.
>   **Every submission attempt returned HTTP 504**, across three tries and a
>   period when the rest of the API was healthy. Submit this when it recovers.
>
> One early JellyCar 3 attempt used a *guessed* identifier
> (`com.walaber.jellycar3`) and also 504'd. If a stray row under that name
> exists, reject it; the correct identifier is `com.disney.JellyCar3`.


Covers all three JellyCar entries on the target list, because they share one
blocker.

- Branch: `compat/jellycar`.
- Artifacts (all `source: original`, all hash-verified):
  - **JellyCar 1.5.4** — <https://archive.org/download/ios3-6-ipas/>,
    `JellyCar 1.5.4 (Decrypted).ipa`, 29,654,461 bytes,
    MD5 `ac69d3e8f4e656023bc7ab213d5d7b3e`,
    SHA-256 `6af50c478f8c44f2505cddbca041d293cd7f10e0dd46574e6e31e9faaf001803`.
    Bundle `com.walaber.jellycar`, version `1.5.4`.
  - **JellyCar 2 on iPad** — <https://archive.org/download/iPad-1-ipa/>,
    `JellyCar 2 on iPad.ipa`, 62,513,270 bytes,
    MD5 `6be32f8a6852cbd3f23e04088c3577ae`,
    SHA-256 `ea95c55c9ded365dcc18f8d498ce0b0b1660bf7766fd8b65a082ba0752c269de`.
  - **JellyCar 3 1.2** — <https://archive.org/download/ios3-6-ipas/>,
    `JellyCar3 1.2 (Decrypted).ipa`, 24,285,094 bytes,
    MD5 `e6de7e8d5d84140967abfa43175b7405`,
    SHA-256 `f2e43f0d678d1d70bf3521c88fe71607241d03303e18541146c1260d44e05d59`.
- tapHLEdb: JellyCar is app 23, version 23, report 31 (2026-07-26, tapHLE
  `9d6ee348`, ★☆☆☆☆). JellyCar 2 and 3 were not launched — see below.

## UPDATE: zlib landed; the blocker below is cleared

The gzip file API is implemented on `trunk` in `d6fde9d1` (gzopen, gzread,
gzwrite, gzclose, gzeof, gztell, gzrewind, gzseek, gzgetc, gzerror), so the
`gzopen` abort described below no longer happens.

**JellyCar 1.5.4 now runs further and fails elsewhere:**

```text
ERROR! no root element in bundle file:.../Documents/scenes.xml
...
Error during CPU execution: MemoryError
```

That message comes from the app's own XML parser, and it is **not** a
compression problem: the bundle contains no `.gz` files at all, only plain
`.scene` and `.softbody` files, so `scenes.xml` is something the app builds in
its Documents directory at first run. It is reading back an empty or malformed
one and then faulting.

Next discriminator: find where `scenes.xml` is written. Trace the file APIs
(`TAPHLE_LOG_MODULES=tapHLE::libc::posix_io,tapHLE::libc::stdio`) on a fresh run
directory and see whether the app writes it at all, writes it empty, or never
gets that far. Only if it writes through `gzwrite` is the new zlib code
implicated — and it should be checked against a real gzip file first, since
nothing has yet exercised the write path end to end.

### JellyCar 3 was launched; it does not share this failure

JellyCar 3 1.2 needed `+[NSBundle pathsForResourcesOfType:inDirectory:]` and
its instance counterpart, both of which were missing (added on `trunk`). With
those it gets past resource enumeration and now faults in guest code with
`Error during CPU execution: MemoryError`, at a different point from JellyCar 1
and with no `scenes.xml` message.

**A hypothesis that was tested and failed:** `pathsForResourcesOfType:` looked
like the obvious way JellyCar 1 would build its scenes list from the bundle's
`.scene` files, so adding it might have fixed both. It did not — JellyCar 1's
behaviour is byte-for-byte unchanged. Whatever writes `scenes.xml` is
something else, so do not re-try that idea.

### JellyCar 2 reaches menus (2 stars)

**JellyCar 2 on iPad, bundle `com.disney.jellyCar2foriPad`, version 1.1**
(iPad, 1024x768). Verified with `tapHLE --info`; note the publisher is Disney,
not Walaber, and the same is true of JellyCar 3 (`com.disney.JellyCar3`).

Three general gaps had to be closed, all on `trunk` in `826f8144`:
`UIPushButton` (a pre-UIButton private class reached through an old nib), a
`UIButton initWithCoder:` assertion that a button must carry
`UIButtonStatefulContent`, and `UISlider`'s missing value/min/max.

With those it reaches the **mode-select screen** (Classic, Factory, Long Jump,
2P Tether, 2P Race, My Levels) and, from Classic, a series of **instruction
cards** showing a car, terrain, a goal flag and drive controls.

**Those cards are not gameplay, and they are not the way in.** Two captures
five seconds apart are byte-for-byte identical, and the timer on them (51.482,
then 49.076 on the next card) is static artwork. The mode screen says it
plainly — "Touch an icon below for **instructions**" — so that whole screen is
the help section, and paging through it only reaches more cards.

### The route to actual gameplay (3 stars)

Backing out of the help section with the bottom-left arrow reveals the **real
main menu**: a spiral notebook with mode icons down the left margin. Play
starts from there.

Click map, 1024x768 client, ~22 s launch wait:

1. First screen -> `(62, 705)` back arrow -> notebook main menu.
2. Menu -> `(62, 622)` CLASSIC mode -> difficulty select.
3. Difficulty -> `(528, 628)` EASY -> level thumbnails.
4. Thumbnails -> `(447, 480)` first level -> a confirm dialog naming the level.
5. Dialog -> `(583, 440)` OK -> **gameplay**.

In gameplay, level "Lance" renders with blue terrain, the orange jelly car,
drive arrows, a pump and a pause button, and the timer runs: 18.720 at first
capture, 69.601 about fifty seconds later, with the level live throughout. That
is the loop starting and persisting.

**Coordinate warning.** The difficulty screen's capture is 768x1024 *portrait*
while the client stays 1024x768 landscape, so client X maps from capture Y
there. The surrounding screens capture 1024x768 and map 1:1. Check the capture
dimensions before converting a position.

Not established: whether a synthetic drive input actually steers the car. The
car shifted slightly between captures, but physics settling explains that just
as well, so no claim is made about driving.

JellyCar 3 1.2 is 1 star; see its own entry above.

## Original (superseded) blocker: no zlib at all

JellyCar 1.5.4 aborts during startup:

```text
Call to unimplemented function _gzopen
```

## The blocker is zlib, and tapHLE has none

`gzopen` is zlib's gzip *file* API. This is not a single missing function:
**tapHLE has no zlib support at all.** There is no `inflate`, no `uncompress`,
and no compression crate in `Cargo.toml` — nothing to build the gz* layer on.

The game reads its level data through it, so nothing renders without it.

JellyCar 2 and 3 were deliberately **not** launched after this was found. They
are the same developer and engine lineage, so they almost certainly need the
same thing, and spending three launches to confirm one blocker three times is
not worth it. Launch them immediately after zlib lands, not before.

## Next step: a bounded zlib file layer

This is a good self-contained `feat/`, and it unblocks up to three apps on the
target list at once:

1. Add a compression crate (`flate2` is the obvious choice and already builds
   on Windows) to `Cargo.toml`.
2. Implement the gz* file API in `libc`: `gzopen`, `gzread`, `gzwrite`,
   `gzclose`, `gzeof`, `gzseek`, `gztell`, `gzerror`. These must go through
   `env.fs`, not the host filesystem, so the guest sandbox still applies —
   that is the part to get right, and the reason this cannot just wrap
   `flate2`'s file helpers directly.
3. Only then relaunch JellyCar 1, and immediately after it JellyCar 2 and 3.

The raw `inflate`/`deflate` stream API is a separate question; do not implement
it speculatively. Check whether the app references those symbols at all before
deciding, since only the gz* file path is known to be needed.

## 2026-07-27: JellyCar 1's fault is a missing `Documents/scenes.xml`

Still 1-star, but the `MemoryError` is no longer opaque.

Two zlib bugs were found and fixed on the way (both on `trunk`, and both wrong
independently of this app):

- `gzopen` returned NULL for a file that is not in gzip format. zlib opens such
  a file and copies its bytes through — `gzread`'s documentation says so — and
  JellyCar opens a plain XML level file this way.
- `gzdirect`, which is how a caller asks which of the two happened, did not
  exist. It surfaced the moment the first fix landed.

### Correction: the XML warning is not the cause

An earlier version of this note said the libxml2 failure was the crash. **That
was wrong**, and the way it was wrong is worth recording: the warning and the
fault are both in the log, so it read as cause and effect. Counting lines shows
the warning at line 85 of 208 — roughly 120 log lines and a good deal of work
before the app dies. It survives the missing XML.

The zlib fixes above still stand on their own terms; they were real bugs. They
just were not this app's blocker.

### Where it actually stops

The last app-level message before the fault is its own:

```text
JellyCar[0] ERROR: DMOAnalytics needs to be initialized with one of its
designated initializers, e.g. initWithURL:appKey:secret:
```

It then writes `Library/Preferences/com.walaber.jellycar.plist`, reads
`Library/Caches/analyticsQueue.plist`, and takes a null-page access at 0x0
(`PC 0x30190`, `R3 0x00000000`).

So the blocker is the bundled **DMOAnalytics** SDK: its shared instance was
never given a URL, app key and secret, and the app then uses it anyway.

### This is the third app on this list to die in an analytics SDK

Mr. Oops!! dies in OAuthConsumer with a nil `OAConsumer`; SPY mouse HD reached
its splash and no further until a guest `NSArray` subclass in its analytics
JSON path was fixed. That is a pattern worth treating as one problem rather
than three: these SDKs are the code most likely to use runtime reflection,
class clusters and network-dependent initialisation, which is exactly where
tapHLE is thinnest.

The next step for this app is to find what should have called
`-initWithURL:appKey:secret:` and why it did not — trace allocation and
selector activity for `DMOAnalytics` from startup. Do **not** start from the
XML; that has now cost one wrong conclusion already.

## 2026-07-27: JellyCar 2 was broken and restored, same day

JellyCar 2's three-star rating stopped reproducing partway through this
session. It aborted during startup with a guest `MemoryError`, having been a
working three-star app since `d335f7bd`.

**Cause: a tapHLE change, not the app.** The layout-on-mount pass added for Tap
Tap Revenge 2 ran `-layoutSubviews` synchronously inside `-addSubview:`, which
executes an app's layout code while it is still assembling its view hierarchy.
Bisected in three builds: alive at `d335f7bd`, alive at `cc492376`, dead at
`b1de9e9e` — the layout-pass merge.

**Fix:** lay out on mount only once launching has finished. Deferring it
entirely also worked for this app but cost Tap Tap Revenge 2 its background
artwork, so neither "always synchronous" nor "always deferred" was right; the
distinction that satisfies both is *when* the mount happens.

### How it went unnoticed

This app is **not in the routine sweep**, so nothing launched it for a dozen
commits. Its rating was a claim about the past being treated as a claim about
the present. See the playbook's "a liveness check is not a regression check".

### What was and was not re-verified

Startup and first-frame rendering are confirmed on the fix (`21a38b72`), with a
993 KB capture matching the pre-regression one byte for byte in size.

**The gameplay loop has now been re-driven** on a clean committed build
(`JellyCar 2 (tapHLE 87acd74a)`, no `-dirty`), following the click map above:
back arrow, CLASSIC, EASY, first level, OK. The level renders with its blue
terrain, the orange jelly car, the drive arrows, the pump and a running timer,
and two captures fourteen seconds apart differ by SHA-256. The three-star rating
is confirmed on current code, not merely inherited from `d335f7bd`.

No new report was filed: the recorded rating is unchanged, and a rerun that
reproduces an existing rating is moderation noise. What changed is the evidence
behind it, which belongs here rather than in the database.


## 2026-07-27: JellyCar 1 advanced to 2 stars — a missing CF constant

The `MemoryError` is **fixed**, and the cause was one unexported CoreFoundation
constant.

Disassembling the faulting PC (`dev-scripts/disasm-guest-fault.py`, added for
this) showed:

```text
0x0003018e  ldr   r3, [r3]        <- r3 = *(a global)
0x00030190  vldr  d7, [r3]        <== FAULT, with R3 = 0
```

The global lives in `__DATA,__nl_symbol_ptr` at `0x433e8`, and walking the
indirect symbol table names it: **`_kCFAbsoluteTimeIntervalSince1970`**. The
code is the standard `CFAbsoluteTimeGetCurrent() + kCFAbsoluteTimeIntervalSince1970`
conversion to a Unix timestamp. tapHLE did not export it, so the slot was null
and the guest loaded a double through address zero.

It is exported now, from the value tapHLE already had
(`SECS_FROM_UNIX_TO_APPLE_EPOCHS`).

**tapHLE had been logging this all along** — `Warning: unhandled non-lazy symbol
"_kCFAbsoluteTimeIntervalSince1970" at 0x433e8` — and the address in that warning
is exactly the one the faulting instruction loads from. See the playbook; the
grep is far cheaper than the disassembly and should be step one.

### Where JellyCar 1 stops now

The main menu renders (logo, `?` button, a JellyCar 3 promo, and EASY / AVERAGE /
HARD difficulty cards). Tapping EASY reaches the level-loading screen with its
"Tilt the device to add rotation to the car!" tip and a progress bar — and stops
there; repeated screenshots are byte-identical.

Behind it, the log shows the two things recorded earlier: libxml2 failing to load
`Documents/scenes.xml`, and `DMOAnalytics needs to be initialized`. Those are now
the frontier rather than a guess, since everything ahead of them is cleared.

Two star: it reaches menus but no gameplay.
