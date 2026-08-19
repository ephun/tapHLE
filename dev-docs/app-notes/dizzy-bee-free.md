# DizzyBeeFree compatibility work note

- Branch: `compat/dizzy-bee-free`. Nothing in the emulator was changed for this
  app; it benefited from `fix/transaction-owns-its-timing-function`, found on
  its sibling.
- Identity, read from `tapHLE --info`: display name `DizzyBeeFree`, bundle
  `com.dizzybeegame.dizzybeefree`, version `1.2`, bundle directory
  `DizzyBeeFree.app`, minimum OS `2.2.1`, no required capabilities, iPhone.
  SHA-256 `69f70d11…2a44ec`.
- A separate app from `Dizzy Bee` and `Dizzy Bee 2`, each with its own note.
- Local copy from the maintainer's collection, not Archive-backed.
- Route: `dev-docs/clickmaps/dizzy-bee-free.json`. No launch options.

## 2026-08-19: three stars on `d60ed8c7`

The file-select screen opens a level and the level plays: the bee rolls where
the device is tipped, the board reacts, and it is still live after half a
minute of steering. Replayed from the committed map on a fresh profile.

The route is the same as its two siblings', which is the whole reason this took
minutes rather than an afternoon: the family shares a menu, a save-slot screen
and a control scheme, so the map transfers and only the board differs.

**The report is blocked** by the database outage — 522 at the root — so both
boundaries are unfiled. Submit what the app holds when it returns.

## Not yet checked

- Completing a level.
- Whether the free build's limits (a level count, an advert) behave.
- Sound.
