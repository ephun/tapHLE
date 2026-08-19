# PapiJump compatibility work note

- Branch: `compat/papijump`. Nothing in tapHLE was changed for this app; it was
  already playable and nobody had looked.
- Identity, read from `tapHLE --info` and not from the filename: display name
  `PapiJump`, bundle `net.sunflat.iphone.PapiJump`, version `1.6.1.0.4`,
  canonical `PapiJump.app`, minimum OS `2.0`, no required capabilities, device
  family iPhone. SHA-256 `7808e179…e69272`.
- The other copy in the collection is a **different app**: `PapiJump 1.0.0` is
  bundle `net.sunflat.iphone.PapiJumpiPad`, canonical `PapiJumpiPad.app`,
  minimum OS `3.2` — the iPad build, and it stops on
  `UISegmentedControl.setSelectedSegmentIndex:`. Do not treat the two as
  versions of one thing.
- Local copies from the maintainer's collection, not Archive-backed.
- Route: `dev-docs/clickmaps/papijump.json`. No launch options.

## 2026-08-19: three stars on `d387265f`

The menu comes up, Start Game begins a round, and the round plays: green
platforms on blue, the red ball bouncing under its own physics, the score drawn
along the top. Replayed twice — once while exploring and once from the
committed map — and in both the ball is at a different height in each capture
fifteen seconds apart, so the loop is live rather than a held frame. The second
run started from a different platform layout and a different score, which is
the game generating a new round rather than replaying a recording.

Nothing had to be fixed. This app was sitting at `ok:survived` in the survey,
which only says it did not fall over in the first thirty seconds unattended;
one tap turned that into a rated result. That is worth saying plainly, because
there are sixty-two other apps in the same state.

**Steering was not exercised.** PapiJump is played by tilting the device, which
tapHLE offers as the right mouse button held while the cursor moves, and a
clickmap cannot express that. So the rating rests on the round running and
persisting, not on climbing; the score stays where the first bounces left it.
Whether tilt steering works here is unmeasured and is the obvious next thing to
check.

**The report is blocked.** `https://taphle.ephun.net/` answers HTTP 522 at its
root and at `/compatibility/api/apps`, as it has all day. Both boundaries this
app crossed — nothing recorded to two stars, and two to three — are therefore
unfiled. Submit the rating it holds when the endpoint returns, on the revision
tested then; do not back-date to `d387265f`.

## Rendering, in one respect, is wrong

The logo at the top of the menu is drawn wider than the screen and is cut off
at both sides — the word is unreadable at its edges. The rest of the menu is
laid out correctly and the game screen is unaffected, so this is not what keeps
the app from a higher rating, but it is a real defect and it is not this app's
alone: another app looked at on the same day draws its whole scene into the
top-left corner of the window. Both are worth chasing together as a
layout-and-scaling question rather than one game at a time.
