# The tapHLE desktop frontend

`tapHLE-gui` is the graphical frontend: a library of installed apps, a details
panel, settings, and an integrated log. This document records how it is built
and, more importantly, why — the decisions here are the ones a later change is
most likely to undo by accident.

Its source is the `tapHLE_gui` package in `src/gui`.

## It is a separate program, not a mode of the emulator

The emulator owns its process. `Environment` maps guest memory at addresses it
chooses, drives an SDL event loop, and ends a run by calling
`std::process::exit`. One of those per process is the design, not an oversight,
and a library window sharing that process would die with every game.

So the frontend launches `tapHLE` as a child process, with the same arguments
somebody would type at a terminal, and reads its output back through pipes.
Four things follow, and they are the reasons for the choice rather than
consequences of it:

- the emulator opens its own window and the library window stays alive;
- a crash in a game cannot take the frontend with it, so the log and the
  diagnostics survive to be read afterwards;
- every option the interface sets is an option the command line already
  accepts, so the two interfaces cannot drift apart — and a settings change
  can be reproduced by hand;
- relaunching costs nothing, which is the whole developer loop.

On Windows the child is created with `CREATE_NO_WINDOW`, so no console appears.
A console object is still allocated — that is what carries the pipes — but it
has no window; `crate::process` is the only place that flag is set.

The two programs are separate binaries for a second reason: subsystems. On
Windows a program is built either for the console subsystem or the windowed
one. `tapHLE.exe` stays a console program, so running it from a terminal
behaves exactly as it always has; `tapHLE-gui.exe` is a windowed program, so it
never opens a console of its own.

## Why egui

The frontend is drawn with [egui](https://github.com/emilk/egui) through
`eframe`, with `rfd` for file dialogs and message boxes.

The requirement that decided it is that the three things carrying this
interface are custom-drawn in *every* toolkit:

- an icon lattice with per-cell selection, badges and two-line elided labels;
- a metadata panel;
- a log viewer holding a few hundred thousand lines that must scroll and
  filter without stuttering.

Qt would draw the first with a `QListView` and a delegate, the third with a
model and a viewport. GTK the same. The work is the same work. What a
retained-mode toolkit would add on top is a second language, a second build
system, and — for Qt — a large installed dependency that everyone building
tapHLE would need. tapHLE is a Rust project whose maintainer is not primarily
a programmer; adding a C++ UI framework to the build is a real, recurring cost.

Against that, egui:

- is pure Rust and builds with `cargo build`, on every desktop platform, with
  nothing installed beyond what tapHLE already needs;
- is high-DPI correct by construction, because it lays out in points and the
  backend supplies the scale factor;
- handles drag-and-drop, multiple windows and a resizable panel layout;
- runs on Android and iOS through the same `winit` backend, which matters for
  the direction the project has taken;
- draws through OpenGL, which the emulator already requires.

What it costs is the thing to be honest about: **egui draws its own widgets, so
a button is not a Windows button.** The frontend narrows that gap deliberately
rather than pretending it does not exist:

- `theme.rs` loads the *system* UI font — Segoe UI on Windows, and the
  platform equivalents elsewhere — so text matches the desktop it is on;
- the palette, the two-pixel corner radius, the hairline dividers and the pale
  blue selection are Windows' own, not egui's defaults;
- `rfd` is used for every file dialog and message box, which is where a drawn
  imitation would be most obvious and most annoying.

A native menu bar through `muda` is the one further step worth considering. It
was not taken here because it introduces a platform-specific event path for a
part of the interface that is not the reason anyone opens the program.

Electron was never a candidate.

### Nothing was copied from another emulator

Dolphin, DuckStation, PPSSPP, RPCS3 and PCSX2 were read for how they arrange a
frontend — a library in the middle, per-game configuration layered over
global, emulation in its own window, a log with severity and subsystem
filters. Those are conventions, not code. **No source from any of them is
incorporated**, so there is no attribution or licence obligation to record. If
that ever changes, the project of origin, the file, its licence and the
modifications go in this section.

## What is where

| File | What it holds |
| --- | --- |
| `main.rs` | Entry point, window geometry, panic hook |
| `app.rs` | State, the frame, and what each action does |
| `ui.rs`, `ui/` | The interface; every part reports an `Action` |
| `library.rs` | Entries, importing, filtering, sorting |
| `metadata.rs` | Reading an app, and the icon cache |
| `settings.rs` | Global defaults, per-app overrides, argument generation |
| `launcher.rs` | Child processes and their outcomes |
| `logstore.rs` | The shared log buffer and line classification |
| `compat.rs` | Compatibility ratings, local and shared |
| `updates.rs` | Release checking |
| `storage.rs` | Where the frontend's own files live |
| `http.rs`, `process.rs`, `timefmt.rs` | Small shared services |

Every part of the interface reports what the person asked for as an `Action`
and the window applies them after it is built. That is not ceremony: an
immediate-mode interface is drawn while the state it describes is borrowed, so
a menu item that removed a library entry mid-draw would be reaching into the
list it is iterating.

## How it reads apps

Through `tapHLE::app_bundle`, a facade added to the emulator for this purpose.
The frontend does **not** parse `Info.plist` itself, because an app whose
identity the library shows differently from the one a run and a compatibility
report use would be worse than useless.

`bundle` and `fs` stay private to the emulator. They are the guest filesystem,
shaped for the emulator's needs and full of guest paths and unit errors;
making them public would make every one of their signatures part of what
tapHLE promises to other programs, and immediately put the emulator's
internals under clippy's public-API lints.

The publisher and genre come from `iTunesMetadata.plist`, the App Store
wrapper beside `Payload/` in an `.ipa`. An app's own `Info.plist` never
records who published it. Nothing is guessed: an app that records no publisher
has none, and the panel leaves the row out.

## Settings, and why unset means unset

Every emulator setting is an `Option`. `None` means "not decided at this
level", and the chain runs:

```
per-app override → global default → tapHLE_options.txt
                 → tapHLE_default_options.txt → the emulator's own default
```

An unset setting emits **no argument at all**. Emitting the emulator's default
instead would silently countermand the per-app entries in
`tapHLE_default_options.txt`, and those entries are what makes several games
work.

This is why the emulator gained an off spelling for every boolean option —
`--windowed`, `--portrait`, `--no-landscape-native` and the rest. A one-way
flag cannot be turned back off by a later layer, so without them a per-app
override could turn something on and never turn it off.

`EmulatorSettings::validate` feeds the generated arguments back through the
emulator's own `Options::parse_argument`. That is what stops the frontend
drifting from what the emulator accepts, and it is what checks the free-text
"extra arguments" box before a run starts rather than after it fails.

## Logging

There is one `LogStore` for the whole frontend. Reader threads write to it,
the frontend writes its own messages to it, and the log panel only reads. That
is what lets output keep arriving while the panel is collapsed, and what keeps
a crashed app's output available after its window has gone.

tapHLE has no structured logging: `log!` prints `module_path!()`, a colon and a
message, and everything else goes through `echo!` unadorned. Rather than
invent a second logging system for the emulator to write to, `logstore::classify`
recovers the structure from the text — the module prefix is exact, the severity
is inferred from the wording. That inference is guesswork and is deliberately
the only place any of it happens.

The panel keeps an incremental index of the lines passing the current filter,
so a new line is tested once when it arrives rather than the whole buffer being
rescanned every frame. Rows are drawn through `ScrollArea::show_rows`, so only
what is on screen is built.

## Compatibility

Reading is complete. `GET /compatibility/api/apps` is a real, public,
credential-free endpoint, so the frontend shows the shared rating beside the
local one, links to an app's record, and can say whether a record already
exists before anybody drafts a report.

**Submitting is not implemented**, and the report window says so rather than
offering a button that does nothing. The database accepts reports from an
agent token belonging to the maintainer; a submission on behalf of an
arbitrary user needs the GitHub sign-in the project has not built. Until it
exists, the report window assembles the exact contents of a report — identity,
versions, build, platform, rating, options, log excerpt — for pasting into the
web form, which is a real workflow rather than a placeholder.

The local rating never touches the database value. They are different things
and the panel labels them so.

## Updates

`updates.rs` really asks GitHub and really filters for tapHLE's own
`taphle-v*` tag namespace, so an inherited touchHLE `v*` tag is not announced
as an update. As of writing **no tapHLE release has been published** — the
releases list is empty and `releases/latest` answers 404 — so the honest
current answer is "there is no release channel yet", and that is what it
reports. The day a release is tagged it starts finding it with no further work.

Nothing is downloaded or installed; the frontend offers to open the releases
page.

## Network requests

`http.rs` is the only place the frontend makes one, and it shells out to
`curl`. Two optional read-only features needed HTTP, which is a poor trade for
linking a TLS stack that would be the largest dependency in the program and
that the emulator has no use for. `curl` ships with Windows 10 and later, with
macOS, and with essentially every Linux distribution. A machine without it
gets no ratings and no update check; both failures are reported and neither is
fatal. `Transport` is a trait so a linked-in client can replace it without
either caller changing.

## Where the frontend's files live

In `tapHLE_frontend/`, beside `tapHLE_sandbox` and `tapHLE_apps`, as indented
JSON meant to be readable and hand-editable:

| File | What it holds |
| --- | --- |
| `library.json` | Entries, per-app overrides, play statistics, local ratings |
| `settings.json` | Global emulator defaults and frontend preferences |
| `state.json` | Window geometry, panel sizes, view mode, last selection |
| `compatibility.json` | The last ratings read from the database |
| `icons/` | Cached icon bitmaps |
| `frontend_log.txt` | Frontend panics, since a windowed program has no console |

All of it is machine-local. None of it is a compatibility claim, and the
library file names the paths of a personal collection, so the directory is
ignored by Git.

Entries are keyed by the app's own identity — bundle identifier and version —
not by path, so moving a file keeps its settings, its rating and its play time.
The version is part of the key because two versions of one app are separate
records in the compatibility database and can need different settings.

## Deliberately not done yet

- **Screenshots.** The goal asked for a screenshot action "where existing
  rendering infrastructure allows it". It does not quite: the emulator's only
  capture path is the agent-testing one in `app-debugging-playbook.md`, armed
  by environment variables and a marker file and firing once per marker, which
  is not something a button can drive. A user-facing action is emulator work —
  capture on demand, write a file, bind a key — belonging on its own branch.
  The compatibility report already has a place for one.
- **Submitting compatibility reports**, as above.
- **A native menu bar**, as above.
- **Dark mode.** `theme.rs` is written as a palette plus a function that
  applies it, so a second palette is the whole change.
- **Device frames.** The emulation window is the emulator's own, so a
  decorative bezel is emulator work. Nothing here forecloses it.
- **A compact/list view** exists but is plainer than the grid.
