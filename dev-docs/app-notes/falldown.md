# FallDown! compatibility work note

- Branch: `compat/falldown`. The fix that got it into a round graduated to
  `trunk` as `fix/insert-past-the-end`.
- Identity, read from `tapHLE --info` and not from the filename: display name
  `FallDown!`, bundle `FallDown` — the identifier really is that bare word —
  version `1.4`, canonical `iFallDown.app`, minimum OS `3.0`, no required
  capabilities, device family iPhone. SHA-256 `c268449b…a6105f`.
- `FallDown! 2` is a separate app in the collection and has not been looked at.
- Local copy from the maintainer's collection, not Archive-backed.
- Route: `dev-docs/clickmaps/falldown.json`. No launch options.

## 2026-08-19: two stars, and a round that always scores 4

The menu draws and holds: the logo in green neon over Play, How to play,
Highscores and More. **1★ → 2★ on `34791fe2`.**

Pressing Play used to end the emulator outright — `insertSubview:atIndex:`
panicked on an index past the end, which this app hands over the moment the
game screen is built. That is fixed generally and is nothing to do with this
app beyond having been found here.

A round now starts and is drawn: platforms scrolling upwards, the ball, the
crush bar across the top. Steering reaches it, and can be seen doing so — with
the tilt held to the left the ball travels to the left wall and stays there.

**It is not three stars, and the reason is worth chasing.** Every round ends
after about three seconds with `Last score: 4`. Not approximately four —
exactly four, in six runs: with no steering at all, with the tilt held hard to
one side, and with gentle alternating tilts that keep the ball near the middle.
A score that does not vary with the input is not a player skill problem.

The obvious explanation to test first is speed: if the game advances its world
per frame rather than per unit of time, and tapHLE runs its frames faster than
the device did, the ball is crushed before any steering could matter, and it is
crushed at the same point every time. That is measurable — count the platform
rows that pass in a second and compare with the device's own pace — and it
would be a general defect rather than this app's, which makes it worth doing
properly rather than guessing.

The report for the two-star boundary is blocked: the database has answered
HTTP 522 at its root all day. Submit what the app holds when it returns; do not
back-date.

## Rendering

The menu is correct. The playing field is drawn on white, with the platforms in
solid green and black outlines, where the menu is black-on-neon; whether that is
the game's own look or a colour problem is unverified and would need a
reference to settle. It is not what is holding the rating back — the round
ending in three seconds is — but do not assume it is right.
