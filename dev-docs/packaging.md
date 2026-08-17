# Packaging tapHLE

What exists, what is scripted but unproven, and what has not been started.
Read `dev-docs/releases.md` for versioning and tagging; this is about turning
a build into something somebody can install.

## Status

| Platform | Builds | Tested | Packaged |
| --- | --- | --- | --- |
| Windows x86_64 | yes | yes, continuously | bundle in CI; installer scripted, not yet built |
| macOS x86_64 | yes | in CI, not played on | bundle script inherited from touchHLE |
| Linux x86_64 | untried | no | no |
| Android | inherited source only | no | no |
| iOS | on `feat/ios-host` | no | no |

Nothing below claims a package works before somebody has produced one.

## Windows

### The bundle

The redistributable directory is what CI uploads and what a release archive
contains:

```sh
cd dev-scripts
./make-windows-bundle.sh ../target/release/tapHLE.exe
```

It takes the emulator and picks up `tapHLE-gui.exe` from beside it, so a build
that has not produced a frontend still yields a working bundle. The result
holds both programs, `tapHLE_dylibs`, `tapHLE_fonts`, `res/icon.png`, the two
options files, `OPTIONS_HELP.txt`, the readme, the changelog and the licence.

That directory is already a complete portable installation: unpack it anywhere
and run either program.

### The installer

`dev-scripts/tapHLE.iss` is an [Inno Setup](https://jrsoftware.org/isinfo.php)
6 script. Build it from a finished bundle:

```powershell
iscc /DBundleDir=..\tapHLE_windows_bundle /DAppVersion=0.3.0-alpha.1 tapHLE.iss
```

**It has not been built or run yet** — Inno Setup is not installed on the
development machine. The script is written and reviewed; treat the first run as
unproven and check the shortcut, the uninstall entry and an upgrade over an
existing installation before publishing one.

It installs **per user**, into `%LOCALAPPDATA%\Programs\tapHLE`, and asks for no
administrator rights. That is not laziness. tapHLE keeps its saved app data, its
library and its options files beside its executables, and a program in
`Program Files` cannot write to its own directory. Installing per user keeps the
portable layout intact, so an installed copy and an unpacked one behave
identically and either can be moved to the other. Anyone who wants a
machine-wide install can ask for one in the wizard.

The installer places both programs. The command line is not a second-class way
to run tapHLE, and `tapHLE.exe` is a usable program on its own.

Uninstalling removes what was installed and nothing else. `tapHLE_apps`,
`tapHLE_sandbox` and `tapHLE_frontend` are the person's own apps, saved games
and library, and Inno Setup does not touch files it did not place. The user's
`tapHLE_options.txt` is installed only if absent, so an upgrade never discards
what somebody put in it.

The executables carry their icon and version properties, attached by
`src/gui/build.rs` through `winresource` from `res/icon.ico`. Regenerate that
file from `res/icon.png` if the artwork changes; it holds the sizes Windows
asks for between 16 and 256 pixels.

Pinning to the taskbar works with the Start menu shortcut as installed; nothing
extra is needed.

## macOS

`dev-scripts/make-macos-bundle.sh` is inherited from touchHLE and produces a
`.app` for the emulator. It predates the frontend and does **not** include it.

A frontend-carrying bundle needs, at minimum: `tapHLE-gui` as the bundle
executable with `tapHLE` beside it in `Contents/MacOS`, an `.icns` icon, an
`Info.plist` naming the bundle identifier and version, and — because
`paths::user_data_base_path` sends a bundled build to a preferences directory
rather than the executable's own — a check that the frontend's `locate_data_dir`
still finds the resources. Distribution outside a personal machine also needs
signing and notarisation, which the project has no certificate for.

## Linux

Not attempted. The frontend's dependencies are the obstacle to be aware of
rather than the emulator's:

- `eframe` needs X11 or Wayland development libraries at build time;
- `rfd` uses GTK 3 by default for its dialogs. Its `xdg-portal` feature is the
  alternative and avoids the GTK dependency, at the cost of needing an async
  runtime;
- the emulator already needs a C++ toolchain, CMake and Boost.

A first attempt should be an AppImage or a tarball of the portable layout,
because both keep the "everything beside the executable" arrangement the
emulator expects. A distribution package that scatters files into `/usr` would
need `paths::user_data_base_path` to learn about XDG directories first.

## Android and iOS

Neither is a current target. The inherited Android source is in `android/`.
The iOS host lives on `feat/ios-host`. Neither has a frontend story, and the
frontend as written assumes a desktop window and a child process, so a mobile
port would share the library and settings model but not the window.
