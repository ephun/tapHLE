# Scoops compatibility work note

- Branch: `compat/scoops`. Reusable fixes graduated to `trunk`.
- Canonical artifact:
  <https://archive.org/download/com.fossilsoftware.appzilla-ios4.0-clutch-2.0.4/>,
  `com.eeenmachine.scoops-iOS3.0-(Clutch-2.0.4).ipa`, `source: original`;
  size 6,488,237 bytes.
- Hashes:
  - MD5: `3ab0adc8149eed3dbe536dcb1edbbc28`
  - SHA-1: `72e52ca87ca577da11c55592732f4286c4861c9c`
  - SHA-256: `3d4ce40592e352da6bbfdd894deac870183cd633a2f9b54c1adc70a8788dc45e`
- Embedded identity (`tapHLE --info`): display `Scoops`, bundle
  `com.eeenmachine.scoops`, version **`2.5.1`** — note the Archive filename says
  nothing about the version, and the item name mentions iOS 3.0, not the app
  version. Minimum OS `3.0`, iPhone. Same developer as Omium.
- tapHLEdb: App 26, version 26, report 34 (2026-07-26, tapHLE `afcc4cf5`,
  ★☆☆☆☆).

## Current state: 1-star, no frame

## Cleared so far

Both general, both on `trunk` in `7a4d2c4b`:

1. `+[NSObject load]` did not exist. The app's `NGPlatform` class implements
   `+load` and ends it with `[super load]`, which failed with "superclass does
   not respond to selector". A no-op root implementation is correct — tapHLE
   drives `+load` itself.
2. `sel_getUid()` did not exist. It is the older name for
   `sel_registerName()`, so it forwards.

## Current frontier

```text
Error during CPU execution: MemoryError
```

A guest memory fault, so this is now a different class of problem from the two
missing-symbol aborts above: the next step is to read the register dump and
stack trace tapHLE prints just before the panic (they were not captured before
context ran out) and resolve the faulting PC in the extracted binary, per the
debugging playbook.

## Next discriminator

Relaunch, capture the register dump, and disassemble around the faulting PC. If
the faulting instruction dereferences a register loaded from a global, check the
`unhandled non-lazy symbol` warnings in the same log first — that pattern
accounted for several startup faults elsewhere in this session, and is much
cheaper to check than a full trace.

## 2026-07-27: unchanged on current code

Still dies during startup with `Error during CPU execution: MemoryError`, on
`73a43594`, after a session that added a great deal of Foundation, UIKit and
CoreGraphics surface. None of it touched this.

Scoops, JellyCar 1 and JellyCar 3 all fail with the same guest `MemoryError` on
the same build, and it is tempting to treat that as one bug. **The register
dumps do not support it**:

```text
JellyCar 1   PC 0x30190   LR 0x30187   R0 0xb3787344 (a float bit pattern)  R3 0
JellyCar 3   PC 0x71aea   LR 0x71a6d   R1 0x001e2988                        R3 1
Scoops       PC 0x8e1b0   LR 0x8e1a0   R1 0x00020a9c                        R2 0
```

Three unrelated fault sites, and since these are three different binaries the
addresses are not comparable in the first place — so a shared symptom here is
close to no evidence of a shared cause. `MemoryError` is simply what tapHLE
reports for *any* bad guest access.

Each needs its own disassembly around its own faulting PC. An earlier version of
this note proposed one investigation for all three; that was optimistic and is
retracted.
