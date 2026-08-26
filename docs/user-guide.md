# Using tapHLE

tapHLE runs 32-bit iOS apps on modern hardware. This page covers installing it,
adding apps, changing settings, and reading the log when something goes wrong.

tapHLE is experimental. Compatibility is specific to an exact app version, and
many apps will not work yet. Check the [compatibility
database](https://taphle.ephun.net/compatibility) before assuming an app should
run.

**tapHLE does not include apps, Apple software, decryption keys, or any other
proprietary material.** You supply your own lawfully obtained files.

## Getting tapHLE

No release has been published yet. Until one is, tapHLE is built from source —
see `docs/development.md`. `docs/platforms.md` records which hosts work today.

A built or unpacked tapHLE is a **portable installation**: everything lives
beside the executables, and the directory can be moved anywhere. If you install
it with the Windows installer once one exists, it installs per user and behaves
identically to an unpacked copy.

## Adding apps

Put `.ipa` files or `.app` bundles in `runtime/apps`, drop them onto the window,
or use **Add App**.

Those files are ignored by Git, so you can keep playtest targets there without
committing or redistributing them. **Never upload an app file or a raw log
anywhere public.**

## Running an app

```powershell
.\tapHLE.exe
```

Select an app and press **Play**. It opens in its own window; the library stays
open, so you can close the app and start another without restarting anything.

If an app stops unexpectedly, tapHLE says so and offers the log rather than
letting its window vanish.

## Settings

**Settings** are global, and any app can override any of them from its own
**Settings…** button.

### Where settings come from

Everything you set, at either scope, is stored in one file:
the runtime settings file, `settings.json`, beside the emulator. The frontend
writes it and the emulator reads it directly, so a run started from a terminal
like one started from the library.

Options are read in layers, and **a later layer wins**:

| Priority | Layer | What it is |
| --- | --- | --- |
| 1 (lowest) | The emulator's own default | Compiled in |
| 2 | Your global settings | `global` in the runtime settings file — applies to every app |
| 3 | `tapHLE_default_options.txt` | Per-app entries that ship with tapHLE and make particular apps work |
| 4 | Your settings for this app | `apps` in the runtime settings file |
| 5 (highest) | The command line | For a run you start yourself |

**The more specific setting wins.** A setting tapHLE ships for one app beats
your general preference, because it is usually there to stop that app drawing
wrongly — 57 apps lock an orientation this way, and a single global
orientation preference would otherwise break all of them at once. Your own
settings for an app still beat everything, so you can always countermand a
shipped default for the app it concerns.

An app's own settings can be keyed two ways. `com.example.game` applies to
every version of that app; `com.example.game@1.2` applies to that build only
and layers over the first, because two versions of one app can need different
settings.

A setting you have not decided at some level emits **nothing at all** rather
than the emulator's default. That matters: emitting a default would silently
countermand the shipped per-app entries that make several apps work.

`tapHLE_options.txt` is the older per-app options file. It is still read, just
after your settings and before the command line, so one you already wrote keeps
working — but nothing writes it any more and new settings do not go there.

Every option that switches something on has a matching option that switches it
off — `--windowed` for `--fullscreen`, `--no-landscape-native` for
`--landscape-native`, and so on. A one-way flag could not be turned back off by
a later layer, so the off spelling is how you countermand something an earlier
layer turned on.

`runtime/OPTIONS_HELP.txt` explains every option in full. It is the same text `--help`
prints — the file is the source, compiled into the binary — so the two can never
disagree.

### Controls

**Settings ▸ Controls** covers how your controller behaves: the analog dead
zone, how far the left stick tilts the device, and how much the right stick's
virtual cursor is steadied.

The virtual cursor is a pointer the right stick moves around the app's screen;
pressing the stick or the right shoulder button taps. Some apps of this era
treat the smallest movement during a tap as a drag, which makes their menus
almost unusable with a stick. **Steady the cursor** exists for that: smoothing
softens sharp movements, and the hold-still radius ignores movement below a few
pixels so a tap stays a tap. The radius is not drawn on screen.

Tilting deserves a note. Apps that steer by tilting read an accelerometer, and
a desktop has none, so the left stick stands in for it — and you can always
tilt by holding the right mouse button and moving. Several racing games are
written for a device held tipped towards you, which is what the resting angles
are for.

**Where each button touches the screen is not here.** That is a fact about one
app's layout, so it lives in that app's own **Settings… ▸ Controls**, under
**Place controls on the screen…**. That opens the app's screen as a canvas:
press **Show the app's screen** and the app starts in its own window, play it
to whatever screen you want to map, then press **Take the picture**. Nothing is
on a timer — the app stays up until you ask — because only you can tell a title
card from the screen the controls belong on. Then drag a marker onto the app's
own button and choose what presses it.

You do not need a controller. A control can be bound to a key instead: select
it, press **Press a key**, and press the key you want. For movement, a stick
zone can be driven by the arrow keys or by W, A, S and D — hold two at once for
a diagonal, the same as a D-pad. A key bound this way stops being typed into
the app, so leave alone anything an app reads as text. F12 always opens the
debugger and cannot be bound. From a terminal these are `--key-to-touch=` and
`--key-dpad-to-touch=`.

### One option worth knowing about

`--landscape-native` fixes landscape apps that render sideways or clipped. Some
apps set their OpenGL viewport to the landscape size and draw directly, rather
than drawing rotated content into a portrait framebuffer. Without this option
they come out rotated or cut off. Warlords HD needs it.

## The log

**View ▸ Log / Output** opens a panel along the bottom carrying the emulator's
own output, with severity, subsystem and text filters.

It is hidden by default and **keeps recording while hidden**, so it is worth
opening after something goes wrong as well as before. A crashed app's output
stays available after its window has gone.

## Where your files live

| Directory | What it holds | Touched by uninstall? |
| --- | --- | --- |
| `runtime/apps` | The app files you added | No |
| `runtime/sandbox` | Guest save data | No |
| `runtime/frontend` | Library, settings and window state, as readable JSON | No |

None of it is touched by an uninstall, and `tapHLE_options.txt` is only installed
if absent, so an upgrade never discards what you put in it.

Library entries are keyed by the app's own identity — bundle identifier and
version — not by path, so moving a file keeps its settings, its rating and its
play time.

## From the command line

The emulator runs on its own:

```powershell
.\tapHLE.exe "C:\path\to\Game.ipa" --landscape-native
```

An app path is required — picking one from a library is the frontend's job.

- `--help` lists every option.
- `--info` prints what an app says about itself without running it.
- `--copyright` prints the bundled licence text.

The frontend passes exactly these options, so anything it does can be reproduced
by hand, and `tapHLE_options.txt` applies to both.

## When an app does not work

1. **Check the [compatibility database](https://taphle.ephun.net/compatibility).**
   The app may have a known rating and a note saying where it stops.
2. **Open the log panel** and look at what the emulator said before it stopped.
3. **Try `--landscape-native`** if the picture is sideways or clipped.
4. **Check the app version.** A rating applies to one exact build; a different
   version of the same app can behave completely differently.

If you want to go further than that, you can point a coding agent at it without
knowing how to program — `docs/compatibility.md` has the whole path, including a
prompt to copy.

## Legal

tapHLE is not affiliated with or endorsed by Apple Inc. iPhone, iOS, iPod, iPod
touch, and iPad are Apple trademarks.

The emulator source is licensed under the Mozilla Public License 2.0. Due to
dependency license compatibility, distributed binaries are licensed under the GNU
General Public License version 3 or later. Bundled dynamic libraries and fonts
have their own notices in `runtime/dylibs` and `runtime/fonts`.
