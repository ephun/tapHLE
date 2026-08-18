# Clickmaps

A clickmap is a replayable record of how to drive one app to a rating
milestone: the launch options, the window size the coordinates belong to, the
waits, and the taps, with a plain-words note of what each step should produce.

They exist because rediscovering a route is the most expensive and least
interesting thing an agent does. Finding that a game's profile slot is at
`(202, 199)` costs a launch, a screenshot, a guess, another screenshot, and a
few thousand tokens of looking at pictures — and it is the same answer every
time. Recording it once means the next run starts by *replaying* and only
spends tokens where the replay stops.

**Try the map before exploring.** If a clickmap exists for the app, or for
another version of it, run `dev-scripts/clickmap.ps1` first. Explore only from
the step where it stopped. This is also the rule for a version bump: a map for
1.0 is usually most of the map for 1.1, and the parts that moved are exactly
what is worth a screenshot.

## Files

- `dev-docs/clickmaps/schema.json` — the format, JSON Schema 2020-12.
- `dev-docs/clickmaps/<slug>.json` — one map per app, slug matching the app
  note in `dev-docs/app-notes/`.
- `dev-scripts/clickmap.ps1` — the runner. `-Validate` checks a map without
  launching anything; `-KeepOpen` leaves the app running at the end and prints
  its process id.

The app note stays the place for narrative: what was tried, what was ruled out,
why a coordinate is where it is. The clickmap is only the route. Keep the note
pointing at the map rather than duplicating the steps, so they cannot drift.

## Replaying

The emulator replays a map itself:

```sh
tapHLE "tapHLE_apps/J & F HD (v1.1) [Cracked].ipa"     --replay=dev-docs/clickmaps/jim-and-frank-hd.json --replay-quit
```

**Prefer this.** It queues each tap as the same event a real mouse click
produces, through the same coordinate transform, so the app cannot tell the
difference — and because no host cursor is involved it behaves identically on
Windows, macOS and Linux, does not need the window in front, does not need an
interactive session it owns, and cannot stray outside the window and click on
something else. A clickmap is only infrastructure if it replays everywhere;
one that runs on a single platform is a script for that platform.

It also removes a whole class of wrong answers. Driving the host cursor on a
display with scaling puts it somewhere other than where it aimed, which is how
four apps were recorded as having unresponsive controls in one session when
nothing was wrong with any of them.

### The PowerShell runner

```powershell
.\dev-scripts\clickmap.ps1 -Map dev-docs\clickmaps\jim-and-frank-hd.json `
                           -App "tapHLE_apps\J & F HD (v1.1) [Cracked].ipa"
```

Windows only, and it drives the real cursor. What it still does that `--replay`
does not is capture a frame after every step, so reach for it when you want
that per-step evidence and are on Windows. Everything else should use
`--replay`.

The runner launches the app, executes each step, captures a frame after every
step into `-OutDir`, and prints one line per step. It exits non-zero if the app
died, and names the step it died on.

A replay drives real synthetic input at the real desktop, so before every tap,
drag or keypress the runner checks that tapHLE is actually in front and, for
pointer steps, that tapHLE is the window under the point. If either check
fails the step fails and nothing is pressed. Treat that failure as a fact
about the machine rather than about the app: something else took the
foreground. Do not remove the check to make a replay finish — a run that
skipped it once left a brush stroke on an unsaved document in another
application.

The frames come from `PrintWindow`, which asks the window to draw itself into a
device context. That is enough for most apps, but an app presenting through
OpenGL ES 2.0 can hand back the last composited content instead of the live
frame — so its captures show a screen it left behind, and a milestone verified
from them would be verified against nothing. Where a map says to use a screen
capture, replay it with `-KeepOpen` and capture the window off the desktop
while it is still up, then close the process. OLO is the current example.

What it does **not** do is decide whether a step worked. It cannot: `expect` is
prose, and comparing frames is what
`dev-docs/app-debugging-playbook.md` warns about — a screen that animates on
its own changes between two frames whether or not the tap landed. So the runner
reports, and a human or an agent looks at the captures. That division is
deliberate: the cheap mechanical part is automated, the judgement is not.

## Recording a map

Write one whenever a run reaches a rating milestone, in the same commit as the
report. The route is fresh then and never will be again.

Record the coordinates in **client** coordinates, origin top-left. A capture
usually includes the window border and title bar, so a position read off a
screenshot is not a client coordinate — subtract the offset, and say so in
`notes` if the app's own frame capture uses different axes again.

Put in `notes` anything a replayer would otherwise have to rediscover the hard
way. Real examples from existing maps: a Return that only registers as a raw
scancode because SDL never sees a virtual-key-only synthetic press; a title
screen that keeps animating for seconds after a missed tap, so a frame
comparison shows change either way; a press shorter than about 300 ms that one
app does not see at all.

## Saved state is why a replay diverges

The most common reason a good map stops working is that the app now has a save.
A game whose first run offers **New Game** offers **Continue** on the second,
in the same place, and the step after that is a different screen.

Mark those steps with `requires_save_state`, and prefer routes that survive
both. When a route cannot, record the fresh-profile route as the map and note
where a saved game diverges — a replay that finds the wrong screen should say
so rather than click on regardless.

## Reporting a replay

A replay is evidence, and the same rules apply as to any other run: identity
comes from `tapHLE --info`, and a result from a dirty worktree never enters the
database. If a replay of a map recorded at three stars now stops at step 4,
that is a regression and it is filed immediately — with the step id in the
frontier text, which is the most useful thing a later reader can be given.

If a map replays clean, say so in the report's frontier. "Replayed
`jim-and-frank-hd.json` end to end" is a stronger and shorter claim than a
paragraph describing the same clicks.
