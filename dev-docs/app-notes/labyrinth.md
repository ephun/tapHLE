# Labyrinth compatibility work note

- Branch: `compat/labyrinth`. No emulator change has come out of it yet.
- Identity, read from `tapHLE --info`: display name `Labyrinth`, bundle
  `se.codify.labyrinth`, version `1.2`, canonical `Labyrinth.app`, no minimum
  OS declared, no required capabilities. SHA-256 `1ff7275f…402a08`.
- `Labyrinth 2`, `Labyrinth 2 HD`, `Labyrinth 2 Lite` and `Labyrinth 2 HD Lite`
  are separate apps by a different developer (`se.illusionlabs.*`) and stop in
  different places. This note is about the original only.
- Local copy from the maintainer's collection, not Archive-backed.

## 2026-08-19: two stars, and the buttons are dead for a findable reason

The welcome screen draws and holds: the wooden maze box, "Welcome to
Labyrinth", the instruction to hold the device parallel to the floor, and a bar
along the bottom showing Play, Settings and Credits. **1★ → 2★ on `d60ed8c7`.**

**None of the three buttons responds**, and the reason is now known rather than
guessed. A temporary log in `-[UIView hitTest:withEvent:]` shows every tap
landing on a `UIToolbar` and stopping there:

```text
hitTest on UIWindow at (28, 459)
hitTest on UIView   at (28, 459)
hitTest on UIToolbar at (28, 23)
```

tapHLE's `UIToolbar` keeps its `items` — `UIBarButtonItem`s, which are not
views — and neither draws nor hit-tests them. So the toolbar is a solid,
invisible rectangle across the bottom of the screen that swallows every touch
that lands on it, and the buttons the player can see there are the app's own
artwork underneath it.

Six taps at different heights along the bar and one in the middle changed
nothing at all, which is what that predicts.

**The fix is not this app's**: a toolbar should lay its items out and send an
item's action when one is pressed. `UIToolbar` was the third most commonest
missing class in the 1501-app survey — 28 apps construct one — so every app
with a real toolbar has the same dead buttons today. It is a feature rather
than a repair, and it needs a layout model (flexible spaces, item widths from
titles and images) that this note is not the place to design.

## Also seen at start-up

The app loads `http://labyrinth.codify.se/GamePage/Labyrinth.html` into a
`UIWebView`, which tapHLE logs and ignores. That is a web page the game shows
somewhere, not the welcome screen — the welcome screen draws — and it is
unrelated to the dead buttons, but it will matter to whatever the Credits
button leads to.

**The report for two stars is blocked** by the database outage (HTTP 522 at the
root all day). Submit what the app holds when the endpoint returns.

## Next step

Make `UIToolbar` interactive, then come back: this game is steered by tilting,
`tilt` exists in clickmaps now, and the whole route to a rating is one button
away.
