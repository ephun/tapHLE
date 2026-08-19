# Super Mega Worm compatibility work note

- Branch: `compat/super-mega-worm`. No reusable fix has come out of it yet.
- Identity, read from `tapHLE --info` rather than the filename. Two builds are
  in the local collection and they are one bundle identifier,
  `com.deceasedpixel.megaworm`:
  - display name `Mega Worm`, version `1.80`, canonical `MegaWorm.app`,
    minimum OS `4.3`, no required capabilities, device family iPhone + iPad.
  - display name `Super Mega Worm`, version `2.0.0`, canonical `megaworm.app`,
    minimum OS `4.3`, requires `armv7` and `gamekit`, iPhone + iPad.
- **Both builds declare minimum OS 4.3**, above the 4.0 tapHLE supports, and
  tapHLE says so at launch. That is not a reason to skip the app — the mission
  is every 32-bit game — but it is a reason to expect gaps that have nothing to
  do with this app.
- Local copies from the maintainer's collection, not Archive-backed.
- Rating: one star. Neither build reaches a menu.
- Not in tapHLEdb.

## 2026-08-19: where 1.80 stops, and what is proven about it

Surveyed at `63119bcf` and traced at `0df4aa08`. The app starts up, loads its
nib, initialises audio and OpenGL ES 1.1 through the GLES1-on-GL2 layer, and
dies during its first frames.

**It stops in the app's own code, not in tapHLE.** The host panic is
`Attempted null-page access at 0x0 (0x1 bytes)` raised by tapHLE's null check
inside `libc::string::strlen`, called from guest code — so tapHLE reports the
guest reading address zero rather than failing itself.

Proven, by a register dump at the call and a scan of the binary's `method_t`
records:

- The caller is the XML parser's `valueOfAttributeNamed:forElement:`, whose
  implementation starts at `0x5f1d8`; the call site is `0x5f210`, and the next
  method begins at `0x5f250`. `R0` is zero at the call, so `strlen` was handed
  a null string.
- The app loads a sprite atlas for every folder under `Sprites/`, reading
  `<folder>/Idle/Atlas.XML` unconditionally. Two folders in its own bundle have
  no `Atlas.XML` at all — `Sprites/Debug/Default/Idle` and
  `Sprites/HUD/HealthProgress/Idle` — and both reads fail.
- **It does not check whether the file is there first.** Instrumenting
  `fileExistsAtPath:` recorded nine existence checks in the whole of start-up,
  none of them for an atlas.
- Each failed read is followed by
  `[(NSMutableDictionary *) setObject: forKey:(null)]`, twice, matching the two
  missing files one for one. Foundation would reject that call on a device;
  tapHLE ignores it and logs.

So the chain is: a missing file, a nil `NSData`, an XML parse of nothing, and a
null pointer walked as if it were an attribute.

## Rejected hypotheses

- **A resource lookup returning nil.** Instrumenting every
  `pathForResource:ofType:inDirectory:` that returns nil produced exactly one
  miss for the whole run: `FigGameView.nib`. That is UIKit's documented probe
  for a nib named after the class with `Controller` stripped, and tapHLE's own
  `get_nib_name` immediately tries `FigGameViewController.nib`, which exists and
  loads. Not the cause.
- **A guest filesystem read returning wrong data.** Every other `Atlas.XML`
  reads at its true length (343, 141, 150, 1263, 18544 bytes and so on). Only
  the two absent files fail.
- **The directory-attribute panic.** Before 2026-08-19 this app died earlier,
  in tapHLE, at the `NSFileModificationDate` unwrap in `file_attributes_common`
  — it asks every sprite folder for its attributes. That is fixed on `trunk`
  (`fix/fs-directory-size`, `fix/fs-directory-modification-time`) and the app
  now runs past it. It was never the cause of this fault.

## Next discriminator

**Find out how the app chooses which folders to load**, because that decides
whether this is tapHLE's problem at all:

- If it walks the `Sprites/` tree with a directory enumerator, then a device
  hits the same two folders and the app is crashing on its own bug, which
  tapHLE can only tolerate rather than fix.
- If it reads a list of atlases from one of the bundle's thirteen property
  lists, then tapHLE handing it the wrong list is the defect, and the fix is in
  the plist reading rather than anywhere near XML.

Log `contentsOfDirectoryAtPath:`, `enumeratorAtPath:` and every plist read
during start-up, in one instrumented build, and the two cases are immediately
distinguishable. Do not tolerate the null in `strlen` to get past this: that
hides which of the two it is.

Version 2.0.0 was not traced. It stops with its own null-page access at `0x0`
after a run of `__cxa_atexit` calls, which is a different frontier.
