# Cut the Rope compatibility work note

- Branch: `compat/cut-the-rope`. The fix it needed graduated to `trunk` as
  `fix/gl-negative-buffer-size`.
- Identity, read from `tapHLE --info` and not from the filename: display name
  `Cut the Rope`, bundle `com.chillingo.cuttherope`, version `1.6`, canonical
  `CutTheRope.app`, minimum OS `3.0`, no required capabilities. SHA-256
  `18bb07cc…fad693`.
- The other file in the collection, `Cut the Rope (v1.0)`, is a **different
  app**: bundle `com.chillingo.cuttheropehd`. It has not been driven.
- Local copy from the maintainer's collection, not Archive-backed.
- Route: `compatibility/clickmaps/cut-the-rope.json`. No launch options.

## 2026-08-19: three stars on `4bb73ce7`

Title screen, box list, level grid, and level 1 open and running: the candy on
its rope, the three star targets, Om Nom below, the star count and MENU across
the top. The scene animates between captures fifteen seconds apart, so it is a
live level rather than a held frame, and the artwork is correct — the cardboard
texture, the lighting on the candy, the star glow.

**What was in the way was tapHLE, not the app.** Opening the level called
`glBufferData` with a size of **-64**, four times, and tapHLE used that size to
take a slice of guest memory: a negative length is impossible, the conversion
panicked, and the emulator died the moment the level was built. OpenGL ES has an
answer for a negative size — `GL_INVALID_VALUE`, the call ignored — so the call
is now passed to the driver to refuse. The level draws correctly with those four
calls refused.

**Where the -64 comes from is not established**, and it is worth establishing.
It is either the app's own arithmetic or tapHLE reading the argument wrongly,
and the two have very different consequences: the second would mean other
`glBufferData` calls are also being read wrongly and merely happen not to go
negative. The next step is to log the guest registers at that call and compare
them with the app's own code at the call site.

**Playing is not exercised.** Cutting a rope is a swipe across it and the route
does not do it, so the rating rests on the level running. Whether a cut rope
drops the candy, whether stars are collected, and whether the level can be
finished are all unmeasured.

**The report is blocked.** The database answered HTTP 522 at its root all day.
Both this boundary and the two-star one it passed on the way are unfiled, and
neither can be reconstructed later — a report asserts that the artifact was run
and rated at that revision, which a later submission cannot honestly claim.
Submit the rating the app holds now once the database is reachable.
