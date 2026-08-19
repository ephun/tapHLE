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

Put `.ipa` files or `.app` bundles in `tapHLE_apps`, drop them onto the window,
or use **Add App**.

Those files are ignored by Git, so you can keep playtest targets there without
committing or redistributing them. **Never upload an app file or a raw log
anywhere public.**

## Running an app

```powershell
.\tapHLE-gui.exe
```

Select an app and press **Play**. It opens in its own window; the library stays
open, so you can close the app and start another without restarting anything.

If an app stops unexpectedly, tapHLE says so and offers the log rather than
letting its window vanish.

## Settings

**Settings** are global, and any app can override any of them from its own
**Settings…** button.

### Where settings come from

Options are read in layers, and **a later layer wins**:

| Priority | Layer | What it is |
| --- | --- | --- |
| 1 (lowest) | The emulator's own default | Compiled in |
| 2 | `tapHLE_default_options.txt` | Per-app entries that ship with tapHLE and make particular apps work |
| 3 | `tapHLE_options.txt` | Your own per-app entries, keyed by the same bundle identifier |
| 4 (highest) | The command line | Including everything the frontend sets — its global defaults and then its per-app overrides |

Both options files are **per-app**: every line is an app's bundle identifier, a
colon, and the options for that app. Neither has a global section, so the only
setting that applies to every app is the frontend's global default, which
reaches the emulator on the command line.

So a per-app override in the frontend beats your `tapHLE_options.txt`, which
beats the shipped per-app defaults, which beat the emulator's own.

A setting you have not decided at some level emits **nothing at all** rather than
the emulator's default. That matters: emitting a default would silently
countermand the shipped per-app entries that make several apps work.

Every option that switches something on has a matching option that switches it
off — `--windowed` for `--fullscreen`, `--no-landscape-native` for
`--landscape-native`, and so on. A one-way flag could not be turned back off by a
later layer, so the off spelling is how you countermand something an earlier
layer turned on.

`OPTIONS_HELP.txt` explains every option in full. It is the same text `--help`
prints — the file is the source, compiled into the binary — so the two can never
disagree.

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
| `tapHLE_apps` | The app files you added | No |
| `tapHLE_sandbox` | Guest save data | No |
| `tapHLE_frontend` | Library, settings and window state, as readable JSON | No |

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
have their own notices in `tapHLE_dylibs` and `tapHLE_fonts`.
