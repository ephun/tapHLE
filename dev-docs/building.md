# Building tapHLE

Windows is tapHLE's primary development and compatibility environment, and its
build instructions are below. macOS builds and is useful for debugging and
behavioural comparison. Linux is an intended desktop platform that nobody has
built yet. Android is not an active target. A modern iOS host is a likely
future target; the experimental one is not on `trunk` and lives on the
`feat/ios-host` branch, with its own build instructions there. `AGENTS.md` has
the honest per-platform state and `packaging.md` says what each would need.

`cargo build` produces two programs: `tapHLE`, the emulator, and `tapHLE-gui`,
the desktop frontend. Both are workspace default members, so an ordinary build
makes both and the lint and format scripts cover both. To build one on its own,
use `cargo build -p tapHLE` or `cargo build -p tapHLE_gui`.

The frontend needs nothing beyond the emulator's own prerequisites on Windows
and macOS. On Linux it would additionally need X11 or Wayland development
libraries for `eframe`, and GTK 3 for `rfd`'s file dialogs. Its design is
described in `gui-architecture.md`.

If you are building to contribute, also read `../CONTRIBUTING.md` and the root
`AGENTS.md`.

## Windows prerequisites

Install:

- [Git](https://git-scm.com/), including Git Bash for the shell scripts;
- [rustup](https://www.rust-lang.org/tools/install), which automatically
  installs the exact Rust version and components pinned in
  `../rust-toolchain.toml`;
- Visual Studio 2022 Build Tools with “Desktop development with C++”;
- [CMake](https://cmake.org/) available on `PATH`; and
- Boost headers.

CMake 4 dropped support for projects declaring a minimum policy version below
3.5, which the vendored SDL, Dynarmic and OpenAL sources all do. `.cargo/config.toml`
sets `CMAKE_POLICY_VERSION_MINIMUM=3.5` so those configure steps succeed; if you
build outside cargo, set it yourself. Without it the failure is
"Compatibility with CMake < 3.5 has been removed" during a from-scratch build,
which reads like a broken checkout rather than a newer CMake — and a release
build that already has its native artifacts will keep working while a debug one
fails, which makes it look like a profile problem instead.

The native Dynarmic dependency currently expects Boost 1.81 in the repository
on Windows. Download Boost 1.81 and extract/rename its top-level directory to
`vendor/boost`, so `vendor/boost/boost/` exists.

Clone and initialize the native submodules:

```powershell
git clone https://github.com/ephun/tapHLE.git
cd tapHLE
git submodule update --init --recursive
```

## Build and run

From a Visual Studio developer PowerShell, or another shell with the compiler
and CMake available:

```powershell
cargo build
cargo run
```

For a release build:

```powershell
cargo build --release
.\target\release\tapHLE.exe --help
```

Running from the repository root lets tapHLE find `tapHLE_dylibs`,
`tapHLE_fonts`, and `tapHLE_default_options.txt`. To run elsewhere, copy those
resources beside the executable. CI uses `dev-scripts/make-windows-bundle.sh`
from Git Bash to assemble `tapHLE_windows_bundle`.

The emulator needs an app path; run `tapHLE-gui` to pick one from a library.
Keep all local playtest files in `tapHLE_apps`, which is the directory the
frontend scans; it is ignored by Git, and app binaries must not be committed or
redistributed. The executable also needs `tapHLE_dylibs`, `tapHLE_fonts`, and
`tapHLE_default_options.txt` beside it (or in the repository root). To run a
game directly:

```powershell
.\target\release\tapHLE.exe "C:\path\to\Game.ipa"
```

Only use decrypted apps authorized for the compatibility task under the source
and artifact rules in `AGENTS.md`. Archived unavailable builds must also follow
`compatibility/README.md`. Never add an app or its extracted assets to Git.

## Checks for contributors

Fast checks that do not require the custom iPhone OS test SDK:

```powershell
cargo metadata --no-deps --format-version 1
cargo fmt --all -- --check
cargo test --workspace --lib
cargo test -- --skip test_app
```

The full integration test rebuilds `tests/TestApp.app` and needs LLVM plus the
custom SDK described in `../tests/README.md`:

```powershell
cargo test
```

The shell helpers also run formatting, Clippy, and documentation checks when
their prerequisites are installed:

```sh
bash dev-scripts/format.sh --check
bash dev-scripts/lint.sh
```

## macOS development build

macOS is useful for comparing guest behavior with host Apple frameworks and
for debugging shared code. It builds in CI and nobody plays games on it, so
treat a macOS result as unverified. Install Rust,
Git, CMake, a C/C++ toolchain, and Boost (for example, `brew install boost`),
then initialize submodules and use the same Cargo commands.

The inherited macOS bundle helper is:

```sh
dev-scripts/make-macos-bundle.sh \
    target/release/tapHLE \
    "$(cargo run --package tapHLE_version)" \
    "$(cargo run --package tapHLE_version -- --branding)"
```

That helper predates the frontend and bundles the emulator only;
`packaging.md` says what a frontend-carrying bundle would need.

macOS failures should not displace Windows game work unless they block a
debugging or iOS-build path needed for that work.

## Troubleshooting

- If CMake cannot find Boost, confirm `vendor/boost/boost/version.hpp` exists.
- If Cargo reports missing native source, rerun
  `git submodule update --init --recursive`.
- If the executable cannot open fonts or dynamic libraries, run it from the
  repository root or copy all three runtime resources beside it.
- If a game fails, preserve the full tapHLE log and follow
  `agent-workflow.md` before implementing unrelated framework stubs.

Generate internal Rust documentation with:

```powershell
cargo doc --workspace --no-deps --open
```
