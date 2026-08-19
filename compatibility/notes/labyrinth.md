# Labyrinth compatibility work note

- Branch: `compat/labyrinth`. No emulator change has come out of it yet.
- Identity, read from `tapHLE --info`: display name `Labyrinth`, bundle
  `se.codify.labyrinth`, version `1.2`, canonical `Labyrinth.app`, no minimum
  OS declared, no required capabilities. SHA-256 `1ff7275f…402a08`.
- `Labyrinth 2`, `Labyrinth 2 HD`, `Labyrinth 2 Lite` and `Labyrinth 2 HD Lite`
  are separate apps by a different developer (`se.illusionlabs.*`) and stop in
  different places. This note is about the original only.
- Local copy from the maintainer's collection, not Archive-backed.
- Route: `compatibility/clickmaps/labyrinth.json` — the two-star milestone, and the
  Play tap that now opens the level-pack list.

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

## 2026-08-19, later: the buttons work, and the next stop is the app's own class

`feat/toolbar-buttons-can-be-pressed` landed and Play opens the level-pack list
— "Demo levels, by Rod Ferguson" — so the dead-button frontier is closed. The
list draws its header and little else; whether the rows below are missing or
simply dark is not yet established.

Tapping the pack stops on something different in kind from everything before
it:

```text
Object … (class "LevelStatusView") does not respond to selector "initGLAndApp"!
```

**Both names belong to the app, not to iPhone OS.** `initGLAndApp` appears
three times in the app's binary and `LevelStatusView` thirty-five, so the
method is there and tapHLE has not attached it to the object it is being sent
to. That is a class-loading gap rather than a missing framework method, and it
is a different kind of problem from a missing selector on a system class:
nothing can be implemented to fix it, only found.

The next discriminator: whether `initGLAndApp` is defined in a category, or on
a superclass — the name suggests a shared GL view base class — that tapHLE
failed to build, leaving `LevelStatusView` parented somewhere shallower than it
should be. `dev-scripts/objc-method-at.py` names the method at an address, and
walking `__objc_classlist` for the two class names says which class really owns
it.

## Next step

Find out why that method is missing, then come back: this game is steered by
tilting, `tilt` exists in clickmaps now, and a rating is one working level
away.
