# How tapHLE works

An orientation for someone about to change the code. Build instructions are in
`docs/development.md`; diagnostic technique is in `docs/debugging.md`.

## High-level emulation

tapHLE does not emulate an iPhone. It emulates *an app's environment*.

A low-level emulator would run iPhone OS itself: the kernel, the drivers, the
system frameworks as shipped. tapHLE instead runs only the app's own 32-bit ARM
code and supplies its own implementations of everything the app calls into —
Foundation, UIKit, OpenGL ES, OpenAL, the Objective-C runtime, libc.

The consequence that matters day to day: **when an app fails, the missing piece
is almost always something tapHLE has not implemented yet**, not a bug in a
faithful reproduction of something Apple wrote. Compatibility work is mostly the
work of implementing the observed contract of one more API.

The guest is 32-bit ARMv6/ARMv7; the host is 64-bit. Every pointer, every
structure layout, and every calling convention crosses that boundary somewhere.

## The two programs

tapHLE builds two binaries, and the split is deliberate.

`tapHLE` is the emulator. It owns its process: `Environment` maps guest memory at
addresses it chooses, drives an SDL event loop, and ends a run by calling
`std::process::exit`. One of those per process is the design.

`tapHLE-gui` is the desktop frontend — app library, details panel, settings,
integrated log. It launches `tapHLE` as a child process with the same arguments
somebody would type at a terminal, and reads its output back through pipes.

Four things follow, and they are the reasons for the choice rather than
consequences of it:

- the emulator opens its own window and the library window stays alive;
- a crash in an app cannot take the frontend with it, so the log and the
  diagnostics survive to be read afterwards;
- every option the interface sets is an option the command line already accepts,
  so the two interfaces cannot drift apart — and a settings change can be
  reproduced by hand;
- relaunching costs nothing, which is the whole developer loop.

On Windows the child is created with `CREATE_NO_WINDOW`, so no console appears. A
console object is still allocated — that is what carries the pipes — but it has no
window; `crate::process` is the only place that flag is set.

They are separate binaries for a second reason: subsystems. On Windows a program
is built either for the console subsystem or the windowed one. `tapHLE.exe` stays
a console program, so running it from a terminal behaves exactly as it always
has; `tapHLE-gui.exe` is a windowed program, so it never opens a console of its
own.

## Where the code lives

| Path | What it holds |
| --- | --- |
| `src/bin.rs`, `src/lib.rs` | Emulator entry point and main control flow |
| `src/app_bundle.rs` | The narrow public reader other tapHLE programs use to learn what an app says about itself |
| `src/options.rs`, `src/paths.rs`, `src/log.rs` | Configuration, host paths, diagnostic output |
| `src/bundle.rs`, `src/mach_o.rs`, `src/dyld.rs`, `src/abi.rs` | Guest app loading, linking, symbols, ABI boundaries |
| `src/cpu.rs`, `src/mem.rs` | Emulated CPU and guest memory |
| `src/objc.rs`, `src/objc/` | Objective-C runtime model |
| `src/frameworks/` | High-level implementations of iPhone OS frameworks |
| `src/libc.rs`, `src/libc/` | C/POSIX compatibility layer |
| `src/window.rs`, `src/gles.rs`, `src/audio.rs` | Host-facing input, graphics, audio |
| `src/fs.rs`, `src/environment.rs` | Guest filesystem and process state |
| `src/gui/` | The `tapHLE_gui` package — the desktop frontend |
| `tests/integration.rs`, `tests/TestApp_source/` | Emulator integration probes |

Guest-visible API names and ABI constants may intentionally use Apple's naming
instead of Rust naming. Check nearby export tables and tests before renaming
them.

Generate the full API documentation with `cargo doc --workspace --no-deps
--open`.

## Symbol binding: host exports usually win

tapHLE's host exports normally take precedence over a guest dylib's definition,
which is right — they are the ones that know about the emulator.

But a *set* of functions sharing hidden state has to come from one place, and the
two binding paths historically disagreed about which: a non-lazy symbol pointer
already preferred a guest dylib's definition, while a lazy stub preferred the
host's. `guest_definition_wins` in `src/dyld.rs` is where that exception is
stated. Extend it only for the same shape of problem, and say why.

`tapHLE_dylibs/` ships real Apple-era libraries (`libgcc_s.1.dylib`,
`libstdc++.6.0.9.dylib`, `libxml2`, `libz`, `libsqlite3`), so an app that links
one gets the genuine implementation loaded as guest code. Before implementing a
stub, check whether the real thing is already present — see `docs/debugging.md`.

## Rendering: which way up

Anything tapHLE draws that has a handedness — text above all — can be turned over
twice on its way to the screen, and it only lands right when both are accounted
for:

1. **The transform.** UIKit lays out y-downward and Core Graphics y-upward, so
   `UIView` installs a y-axis flip around every `-drawRect:` call. Read it as the
   sign of `CGContextGetCTM(...).d`.
2. **The destination.** The compositor draws a `CALayer`'s backing bitmap with
   its vertical texture coordinate inverted, so everything in it is turned over
   once more (`composition.rs`, `rows_are_top_to_bottom`). A bitmap an app
   created itself and uses as a texture or assigns to `contents` is not.

Two flips cancel. So drawing has to be turned back over exactly when the two
**agree**, and left alone when they differ. Nothing about a bitmap's contents
distinguishes the two destinations, so `CALayer` marks the one it owns and
`CGBitmapContextData::flipped_on_presentation` carries the answer.

**The trap is that the transform alone looks like it explains everything.** It is
the visible half, it correlates with the common case, and a fix keyed to it
passes the app in front of you. It was wrong twice:

- Keying on the CTM alone fixed `-drawRect:` text and left every string an app
  drew into its own bitmap mirrored — and newly mirrored strings that had been
  correct, in apps that flip their own context first, which is the right way to
  draw UIKit text into a bitmap.
- Applying the same band flip inside `CGContextShowGlyphsAtPoint` broke text that
  was already correct and fixed nothing, because it moved the same wrong question
  one layer down.

Two rules follow, and they generalise past text:

- **Ask where the pixels are going, not only how they are being transformed.**
  Handedness is a property of the whole path to the screen.
- **Check both destinations before believing a fix.** One app whose text comes
  right is a sample. A layer-drawn label (`UILabel`, a `-drawRect:` view) and an
  app-owned bitmap are different halves of the same question, and a change that
  fixes one can silently invert the other. Tap Tap Revenge 3's menu against OLO's
  menu is a cheap pair to check against.

The same reasoning applies to the *layout* being mirrored rather than the glyphs:
flip about the band the text occupies, never per glyph. A per-glyph mirror turns
the letters the right way up and leaves the lines of a paragraph stacked upwards,
which looks like a font bug and is not one.

## Layout: current behaviour and its gaps

A view created after launch can be mounted, touchable, and still blank if its
custom `layoutSubviews` never runs.

The current behaviour lays out a newly mounted controller root **once, and only
after launching has finished.** That condition is narrower than it looks and was
arrived at the hard way: laying out on mount unconditionally broke JellyCar 2
while it was what got Tap Tap Revenge 2 into gameplay, and neither restricting
the recursion nor restricting it to `CAEAGLLayer`-backed views explained both
apps. The distinction that did was *when* the layout ran, not *which views* it
touched.

Nested runtime views and later geometry changes still need a dirty-layout
implementation. Do not paper over a missing layout pass by calling every view's
layout method every frame; that can make an app destroy and recreate its
framebuffer continuously.

## The frontend

Source is the `tapHLE_gui` package in `src/gui`.

### Why egui

Drawn with [egui](https://github.com/emilk/egui) through `eframe`, with `rfd` for
file dialogs and message boxes.

The requirement that decided it is that the three things carrying this interface
are custom-drawn in *every* toolkit:

- an icon lattice with per-cell selection, badges and two-line elided labels;
- a metadata panel;
- a log viewer holding a few hundred thousand lines that must scroll and filter
  without stuttering.

Qt would draw the first with a `QListView` and a delegate, the third with a model
and a viewport. GTK the same. The work is the same work. What a retained-mode
toolkit would add on top is a second language, a second build system, and — for
Qt — a large installed dependency that everyone building tapHLE would need.
tapHLE is a Rust project whose maintainer is not primarily a programmer; adding a
C++ UI framework to the build is a real, recurring cost.

Against that, egui is pure Rust and builds with `cargo build` on every desktop
platform with nothing installed beyond what tapHLE already needs; is high-DPI
correct by construction, because it lays out in points and the backend supplies
the scale factor; handles drag-and-drop, multiple windows and a resizable panel
layout; runs on Android and iOS through the same `winit` backend, which matters
for the direction the project has taken; and draws through OpenGL, which the
emulator already requires.

What it costs is the thing to be honest about: **egui draws its own widgets, so a
button is not a Windows button.** The frontend narrows that gap deliberately
rather than pretending it does not exist. `theme.rs` loads the *system* UI font —
Segoe UI on Windows, and the platform equivalents elsewhere — so text matches the
desktop it is on; the palette, the two-pixel corner radius, the hairline dividers
and the pale blue selection are Windows' own, not egui's defaults; and `rfd` is
used for every file dialog and message box, which is where a drawn imitation
would be most obvious and most annoying.

A native menu bar through `muda` is the one further step worth considering. It
was not taken because it introduces a platform-specific event path for a part of
the interface that is not the reason anyone opens the program.

Electron was never a candidate.

### Nothing was copied from another emulator

Dolphin, DuckStation, PPSSPP, RPCS3 and PCSX2 were read for how they arrange a
frontend — a library in the middle, per-app configuration layered over global,
emulation in its own window, a log with severity and subsystem filters. Those are
conventions, not code. **No source from any of them is incorporated**, so there
is no attribution or licence obligation to record. If that ever changes, the
project of origin, the file, its licence and the modifications go in this
section.

### What is where

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

Every part of the interface reports what the person asked for as an `Action` and
the window applies them after it is built. That is not ceremony: an
immediate-mode interface is drawn while the state it describes is borrowed, so a
menu item that removed a library entry mid-draw would be reaching into the list
it is iterating.

### How it reads apps

Through `tapHLE::app_bundle`, a facade added to the emulator for this purpose.
The frontend does **not** parse `Info.plist` itself, because an app whose
identity the library shows differently from the one a run and a compatibility
report use would be worse than useless.

`bundle` and `fs` stay private to the emulator. They are the guest filesystem,
shaped for the emulator's needs and full of guest paths and unit errors; making
them public would make every one of their signatures part of what tapHLE promises
to other programs, and immediately put the emulator's internals under clippy's
public-API lints.

The publisher and genre come from `iTunesMetadata.plist`, the App Store wrapper
beside `Payload/` in an `.ipa`. An app's own `Info.plist` never records who
published it. Nothing is guessed: an app that records no publisher has none, and
the panel leaves the row out.

### Settings, and why unset means unset

Every emulator setting is an `Option`. `None` means "not decided at this level".
See "Where settings come from" in `docs/user-guide.md` for the resolved
precedence.

An unset setting emits **no argument at all**. Emitting the emulator's default
instead would silently countermand the per-app entries in
`tapHLE_default_options.txt`, and those entries are what makes several apps work.

The type itself lives in the emulator, as `tapHLE::settings::EmulatorSettings`,
not in the frontend. Both programs have to agree about what a setting is, and
two definitions of one type is how they stop agreeing.

### One store, read by both programs

`tapHLE_settings.json` holds `global` and `apps`, and belongs to neither
program: the frontend writes it, the emulator reads it directly at startup.

It replaced an arrangement where the frontend kept its settings in
`tapHLE_frontend/settings.json` and `library.json` and handed them to the
emulator as command-line arguments, while the emulator separately read
`tapHLE_default_options.txt` and `tapHLE_options.txt` underneath. The two
systems met only at argv, and because an unset setting emits no argument, a
setting the frontend left alone fell through to a file the frontend never
showed. A run started from a terminal and the same run started from the library
could resolve differently.

Two consequences worth keeping:

- **The frontend saves before it launches.** Saving is otherwise throttled, and
  the emulator reads the file rather than an argument list, so a setting
  changed a moment before pressing Play has to be on disk first.
- **The per-app key is `bundle identifier@bundle version`**, the same string
  the library uses for an entry. `AppMetadata::stable_id` and
  `SettingsFile::app_version_key` build it in different crates from the same
  two fields, and a test in `metadata.rs` is what keeps them identical — if
  they drifted, the frontend would write settings under one key and the
  emulator would look under another, and every per-app setting would quietly
  stop applying while still appearing set.

This is why the emulator gained an off spelling for every boolean option —
`--windowed`, `--portrait`, `--no-landscape-native` and the rest. A one-way flag
cannot be turned back off by a later layer, so without them a per-app override
could turn something on and never turn it off.

`EmulatorSettings::validate` feeds the generated arguments back through the
emulator's own `Options::parse_argument`. That is what stops the frontend
drifting from what the emulator accepts, and it is what checks the free-text
"extra arguments" box before a run starts rather than after it fails.

### Logging

There is one `LogStore` for the whole frontend. Reader threads write to it, the
frontend writes its own messages to it, and the log panel only reads. That is
what lets output keep arriving while the panel is collapsed, and what keeps a
crashed app's output available after its window has gone.

tapHLE has no structured logging: `log!` prints `module_path!()`, a colon and a
message, and everything else goes through `echo!` unadorned. Rather than invent a
second logging system for the emulator to write to, `logstore::classify` recovers
the structure from the text — the module prefix is exact, the severity is inferred
from the wording. That inference is guesswork and is deliberately the only place
any of it happens.

The panel keeps an incremental index of the lines passing the current filter, so
a new line is tested once when it arrives rather than the whole buffer being
rescanned every frame. Rows are drawn through `ScrollArea::show_rows`, so only
what is on screen is built.

### Compatibility

Reading is complete. `GET /compatibility/api/apps` is a real, public,
credential-free endpoint, so the frontend shows the shared rating beside the
local one, links to an app's record, and can say whether a record already exists
before anybody drafts a report.

**Submitting is not implemented**, and the report window says so rather than
offering a button that does nothing. The database accepts reports from an agent
token belonging to the maintainer; a submission on behalf of an arbitrary user
needs the GitHub sign-in the project has not built. Until it exists, the report
window assembles the exact contents of a report — identity, versions, build,
platform, rating, options, log excerpt — for pasting into the web form, which is a
real workflow rather than a placeholder.

The local rating never touches the database value. They are different things and
the panel labels them so.

### Updates

`updates.rs` really asks GitHub and really filters for tapHLE's own `taphle-v*`
tag namespace, so an inherited touchHLE `v*` tag is not announced as an update.
As of writing **no tapHLE release has been published** — the releases list is
empty and `releases/latest` answers 404 — so the honest current answer is "there
is no release channel yet", and that is what it reports. The day a release is
tagged it starts finding it with no further work.

Nothing is downloaded or installed; the frontend offers to open the releases
page.

### Network requests

`http.rs` is the only place the frontend makes one, and it shells out to `curl`.
Two optional read-only features needed HTTP, which is a poor trade for linking a
TLS stack that would be the largest dependency in the program and that the
emulator has no use for. `curl` ships with Windows 10 and later, with macOS, and
with essentially every Linux distribution. A machine without it gets no ratings
and no update check; both failures are reported and neither is fatal. `Transport`
is a trait so a linked-in client can replace it without either caller changing.

### Where the frontend's files live

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

All of it is machine-local. None of it is a compatibility claim, and the library
file names the paths of a personal collection, so the directory is ignored by
Git.

Entries are keyed by the app's own identity — bundle identifier and version — not
by path, so moving a file keeps its settings, its rating and its play time. The
version is part of the key because two versions of one app are separate records
in the compatibility database and can need different settings.

### Deliberately not done yet

- **Screenshots.** The emulator's only capture path is the agent-testing one in
  `docs/debugging.md`, armed by environment variables and a marker file and
  firing once per marker, which is not something a button can drive. A
  user-facing action is emulator work — capture on demand, write a file, bind a
  key — belonging on its own branch. The compatibility report already has a place
  for one.
- **Submitting compatibility reports**, as above.
- **A native menu bar**, as above.
- **Dark mode.** `theme.rs` is written as a palette plus a function that applies
  it, so a second palette is the whole change.
- **Device frames.** The emulation window is the emulator's own, so a decorative
  bezel is emulator work. Nothing here forecloses it.
- **A compact/list view** exists but is plainer than the grid.

## Mobile frontends

Neither Android nor iOS has a frontend, and that is the substantial piece of work
remaining before the first release. The desktop frontend assumes a window it owns
and an emulator it launches as a child process, and neither assumption holds on a
phone: there is no second process to spawn, and the OS owns the window.

A mobile frontend therefore shares the library model, the settings model and the
compatibility-database client, but not the process model or the window. See
`docs/maintaining.md` for what each owes.
