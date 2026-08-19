# Dizzy Bee 2 compatibility work note

- Branch: `compat/dizzy-bee-2`. The fix it turned up graduated to `trunk` as
  `fix/transaction-owns-its-timing-function`.
- Identity, read from `tapHLE --info`: display name `Dizzy Bee 2`, bundle
  `com.dizzybeegame.dizzybee2`, version `1.2`, bundle directory
  `Dizzy Bee 2.app`, minimum OS `2.2`, no required capabilities, iPhone.
  SHA-256 `71b028cf…e72b69`.
- A separate app from `Dizzy Bee` and `DizzyBeeFree`, which have their own
  bundle identifiers and their own notes.
- Local copy from the maintainer's collection, not Archive-backed.
- Route: `dev-docs/clickmaps/dizzy-bee-2.json`. No launch options.

## 2026-08-19: three stars on `d60ed8c7`

The file-select screen opens a level, and the level plays: the bee rolls where
the device is tipped and the board changes as it goes, still live after half a
minute of steering. Replayed from the committed map **on a fresh profile**.

**Getting there needed a real fix, and this app is how it was found.** On a
fresh profile the game shows two tutorial alerts, and shortly after them tapHLE
died on a null dereference. The cause was nowhere near the alerts: every
`CATransaction` was releasing a `CAMediaTimingFunction` it had never retained,
which dropped the cache's only reference, and a later transaction was handed
freed memory. Fixed on `trunk`; the app then plays through.

That is worth remembering as a shape: **a crash a few frames after an
animation, on a first launch only, with an over-release warning somewhere
earlier in the log.** The warning names the object that was over-released, not
the code that did it.

**The report is blocked** — the database has answered HTTP 522 at its root all
day — so both boundaries are unfiled. Submit what the app holds when it
returns; do not back-date.

## Not yet checked

- Completing a level, and whatever follows one.
- The tutorial alerts themselves: tapHLE cannot draw an alert, and reports these
  as dismissed by no button because they have several and none is a cancel. The
  game carries on regardless, but it never gets to say what it was trying to
  teach.
- Sound.
