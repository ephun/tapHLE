# Dizzy Bee compatibility work note

- Branch: `compat/dizzy-bee`. Nothing in the emulator was changed for this app.
- Identity, read from `tapHLE --info` and not from the filename: display name
  `Dizzy Bee`, bundle `com.dizzybeegame.dizzybee`, version `1.5`, bundle
  directory `Dizzy Bee.app`, minimum OS `2.2.1`, no required capabilities,
  device family iPhone. SHA-256 `f85283ff…1ae105`.
- `Dizzy Bee 2` (`com.dizzybeegame.dizzybee2`) and `DizzyBeeFree`
  (`com.dizzybeegame.dizzybeefree`) are separate apps in the collection. Both
  survive start-up and draw the same kind of file-select screen; neither has
  been driven further.
- Local copy from the maintainer's collection, not Archive-backed.
- Route: `compatibility/clickmaps/dizzy-bee.json`. No launch options.

## 2026-08-19: three stars on `3b105947`

The file-select screen comes up, the first slot opens a level, and the level
plays: a honeycomb board with flowers, gates and a spinner, and the bee rolling
where the device is tipped. Replayed from the committed map on a clean build,
with the board changing between every capture across half a minute of steering
and the bee ending somewhere it did not start.

**Nothing had to be fixed, but it could not have been shown before today.** The
game is steered by tilting, and until `infra/clickmap-tilt` a recorded route
could only tap — so the board came up and then sat there, which is
indistinguishable from a game that does not work. The route uses `tilt`, which
holds the right mouse button at a point; tapHLE reads that as the accelerometer.

That is the useful general lesson here, and it is why this note exists for an
app that needed no code: **an app that looks inert may be waiting for an input
the route cannot express.** Anything in the collection that steers by tilting —
and there are many — should be re-driven with `tilt` before it is written off.

**The report is blocked.** `https://taphle.ephun.net/` has answered HTTP 522 at
its root all day, so both of this app's boundaries are unfiled. Submit what it
holds when the endpoint returns; do not back-date to `3b105947`.

## Not yet checked

- Whether the level can be completed — the route steers, it does not play well.
  Reaching the hive, the flower count and whatever follows a finished level are
  all unmeasured.
- Sound.
- The two sibling apps above.
