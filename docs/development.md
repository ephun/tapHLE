# Developing tapHLE

Everything needed to check out, build, test and submit a change. If you want to
make one specific app work, read `docs/compatibility.md` after this. If you want
to know how tapHLE is put together, read `docs/architecture.md`.

Per-platform status lives in `docs/platforms.md`. The instructions below are
written for Windows because that is where tapHLE is developed; macOS notes
follow, and Linux is built in CI but has not been run by anyone.

## What you get

`cargo build` produces two programs:

- `tapHLE` — the emulator. Takes an app path and runs it.
- `tapHLE-gui` — the desktop frontend: app library, settings, integrated log.
  It launches the emulator as a child process.

Both are workspace default members, so an ordinary build makes both and the lint
and format scripts cover both. To build one on its own, use `cargo build -p
tapHLE` or `cargo build -p tapHLE_gui`.

## Prerequisites

### Windows

Install:

- [Git](https://git-scm.com/), including Git Bash for the shell scripts;
- [rustup](https://www.rust-lang.org/tools/install), which automatically
  installs the exact Rust version and components pinned in
  `rust-toolchain.toml`;
- Visual Studio 2022 Build Tools with "Desktop development with C++";
- [CMake](https://cmake.org/) available on `PATH`; and
- Boost headers.

The native Dynarmic dependency currently expects Boost 1.81 in the repository on
Windows. Download Boost 1.81 and extract/rename its top-level directory to
`vendor/boost`, so `vendor/boost/boost/` exists.

**CMake 4 needs a policy override.** It dropped support for projects declaring a
minimum policy version below 3.5, which the vendored SDL, Dynarmic and OpenAL
sources all do. `.cargo/config.toml` sets `CMAKE_POLICY_VERSION_MINIMUM=3.5` so
those configure steps succeed; if you build outside cargo, set it yourself.
Without it the failure reads "Compatibility with CMake < 3.5 has been removed"
during a from-scratch build, which looks like a broken checkout rather than a
newer CMake — and a release build that already has its native artifacts will
keep working while a debug one fails, which makes it look like a profile
problem instead.

### macOS

Install Rust, Git, CMake, a C/C++ toolchain, and Boost (for example, `brew
install boost`). macOS is useful for comparing guest behavior against host Apple
frameworks and for debugging shared code. It builds in CI and nobody plays apps
on it, so treat a macOS result as unverified. macOS failures should not displace
app compatibility work unless they block a debugging or iOS-build path needed
for that work.

### Linux

CI installs these on `ubuntu-latest`:

```sh
cmake ninja-build libboost-dev
libasound2-dev libpulse-dev libudev-dev libdbus-1-dev
libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxi-dev
libxinerama-dev libxss-dev libxkbcommon-dev
libwayland-dev wayland-protocols libdecor-0-dev
libgl1-mesa-dev libegl1-mesa-dev libgtk-3-dev
```

Most of that is SDL2's build dependencies rather than tapHLE's, because the
default `static` feature builds SDL2 and OpenAL Soft from source. Boost is for
Dynarmic. GTK 3 is for the frontend's native file dialogs through `rfd`, which
is why even a headless build needs it.

## Get the source and build

```powershell
git clone https://github.com/ephun/tapHLE.git
cd tapHLE
git submodule update --init --recursive
cargo build
```

From a Visual Studio developer PowerShell, or another shell with the compiler
and CMake available. For a release build:

```powershell
cargo build --release
.\target\release\tapHLE.exe --help
```

Running from the repository root lets tapHLE find `tapHLE_dylibs`,
`tapHLE_fonts`, and `tapHLE_default_options.txt`. To run elsewhere, copy those
resources beside the executable, or use
`dev-scripts/make-windows-bundle.sh` from Git Bash to assemble a complete
portable directory.

The emulator needs an app path; run `tapHLE-gui` to pick one from a library.
Keep local playtest files in `tapHLE_apps`, which is the directory the frontend
scans. It is ignored by Git, and app binaries must not be committed or
redistributed.

```powershell
.\target\release\tapHLE.exe "C:\path\to\Game.ipa"
```

Only use decrypted apps authorized for the compatibility task under the source
and artifact rules in `AGENTS.md`, and the availability rules in
`docs/compatibility.md`. Never add an app or its extracted assets to Git.

## The test ladder

Stop at the highest level you can afford and say where you stopped.

| Level | Command | Needs |
| --- | --- | --- |
| 1. Rust unit tests | `cargo test --workspace --lib` | nothing extra |
| 2. Everything but the guest app | `cargo test -- --skip test_app` | nothing extra |
| 3. Full integration | `cargo test` | LLVM and the custom SDK, see `tests/README.md` |
| 4. Lint and format | `bash dev-scripts/lint.sh` | clang-format |
| 5. The actual app | release build, launch the exact target | the app, on the claimed host |

Level 5 is the only proof that a compatibility claim is true, and it is
host-qualified: running an app on Windows says nothing about macOS.

**Run `dev-scripts/lint.sh` before you merge.** It is the real gate. `cargo fmt`
and `cargo test` can both pass while CI is red, because lint checks things they
do not — including the 80-column rule on `//` comment lines.

```sh
bash dev-scripts/format.sh --check
bash dev-scripts/lint.sh
```

A quick metadata sanity check that catches a malformed manifest early:

```powershell
cargo metadata --no-deps --format-version 1
```

`tests/README.md` is the canonical reference for the TestApp fixture: what it
is, which Clang version CI uses, where the custom SDK goes, and the checksums it
is pinned to.

## Choosing what to work on

Two kinds of contribution move tapHLE forward, and they are different work:

**Specific app work** helps one app a lot, and sometimes a couple of others by
accident. `docs/compatibility.md` is the protocol.

**Broad framework work** implements iOS classes and APIs that many apps are
waiting on. It helps a lot of apps a little.

If you have a large collection of IPAs, the highest-value thing you can do is
**not** to pick a favourite and chase it. It is to measure where every app in
the collection stops, so the project can fix the causes that block the most
apps.

```
cargo build --release
python dev-scripts/survey.py run --apps "D:\path\to\your ipas"
python dev-scripts/survey.py rank
python dev-scripts/survey.py rank --symbols
```

This launches each app briefly, records where it stopped, and ranks the causes
by how many apps hit them. It is resumable, so a large collection can be left
running overnight and picked up later.

**The result file is yours and must never be committed or uploaded.** It lists
the apps you own. The default path is outside the repository and `.gitignore`
covers the pattern; share the *findings*, not the file. A useful contribution
looks like:

> Surveyed 900 apps. Top causes: 40 want `-[UIAlertView setMessage:]`, 31 die on
> an unbound `UIApplicationLaunchOptionsURLKey`, 22 hit the same assertion at
> `src/objc/messages.rs:402`.

That is an issue anyone can act on, and it discloses nothing about your library.

A survey row is a few seconds of unattended running. It is **not** a
compatibility rating and must not be filed as one — nothing in it presses a
button, so an app waiting on a tap looks stuck.

### What the collection asks for

The survey needs a build and runs every app. `dev-scripts/demand.py` needs
neither — it reads the import tables out of each binary and subtracts what `src/`
exports, which takes minutes rather than a night:

```
python dev-scripts/demand.py scan --apps "D:\path\to\your ipas"
python dev-scripts/demand.py todo
python dev-scripts/demand.py app "some game"
```

The two answer different questions. A survey says where an app stopped, which is
what to fix next but reveals nothing about what lies behind it — the remaining
work stays unknown until the last app runs. `demand.py` measures the whole
backlog up front, so it can say which framework is worth starting, how much of
one already exists, and which app is *closest* to running rather than merely
furthest along today.

Its counts are references, not calls: an app that links CoreLocation may never
reach the line that needs it. The same privacy rule applies — the file lists your
collection, it is gitignored, and the findings are what you share.

If you have run both tools over the same collection, `cross` joins them into a
single priority order:

```
python dev-scripts/demand.py cross
```

Every gap is labelled `BLOCKING` when some app's run stopped exactly there,
`reachable` when tapHLE failed to bind it at load but something else stopped the
app, and `latent` when nothing has reached it yet. That separates work that is
proven to block from work that merely looks large — a symbol with high demand and
no blocking evidence is behind the current frontier, not in front of it.

Counts are a floor. A survey older than your working tree under-reports, because
anything implemented since has already been dropped from the static gap list.

## Development workflow

1. Create a focused branch from `trunk`; use `compat/<app-slug>` for app work.
2. Initialize submodules with `git submodule update --init --recursive`.
3. Reproduce the failure or create a small synthetic probe.
4. Make the smallest complete change that advances the target.
5. Add or update a focused test when practical.
6. Run the relevant checks from `AGENTS.md`.
7. For verified app testing, submit the exact result to the compatibility
   database when publication is authorized.
8. Open a pull request using the repository template.

Pull requests should say which agent or AI tool materially assisted, what
evidence guided the implementation, what was tested and on which host, and which
claims still need manual app validation. AI involvement is not a negative;
transparent validation makes the result easier to trust and continue.

Version bumps, release tags, and packages follow `docs/maintaining.md`. Do not
create or move a release tag as part of an ordinary contribution.

### Review standard

Review is outcome-focused:

- Does this improve or protect a target app?
- Is the behavior supported by a reproduction, log, probe, or test?
- Is any shortcut bounded and understandable?
- Are proprietary artifacts absent and source provenance acceptable?
- Were the relevant checks actually run?

Small follow-up improvements are preferable to holding a working,
well-contained fix for an unrelated cleanup.

Pragmatic fixes are welcome. If an app needs a narrow workaround, keep it local,
document the evidence behind it, and add a regression check when practical. A
clean general implementation is preferred when it takes similar effort, but
contributors are not required to redesign a subsystem before shipping a useful
compatibility improvement.

## Copyright and reverse engineering

Compatibility work must not compromise the project legally.

- Prefer public API documentation and clean behavioral experiments.
- You may inspect a target app for an authorized compatibility task under
  `docs/compatibility.md`, but do not commit or redistribute proprietary
  material.
- Do not consult leaked Apple source, private SDK material, or decompiled
  proprietary iPhone OS implementations.
- Do not copy code merely because it is visible online. Check its license and
  preserve attribution and notices when reuse is compatible.
- Describe non-obvious external sources in the pull request and, when useful, in
  a nearby comment.

These rules govern intentional research and submitted artifacts. The project does
not presume that an agent's opaque training history is knowable; it does require
contributors to review generated code and reject suspicious or unverifiable
copying.

## Code style

This section exists to give you an idea of what code in tapHLE should look like,
and where possible, why.

tapHLE is a bridge between worlds: Objective-C versus Rust, guest code versus
host code, 32-bit versus 64-bit, iPhone OS-exclusive APIs versus cross-platform
open-source stuff. This is true even for components written purely in one
language. The coding style is necessarily a compromise between these worlds.

### Formatting

Rust code is formatted with the standard Rust style and `rustfmt` can be used to
reformat it. C and C++ code is formatted with the default style for
`clang-format`. `dev-scripts/format.sh` will run both formatters.

Unfortunately, **`rustfmt` does not understand tapHLE's Objective-C macros**, so
code inside `objc_classes!` and use of `msg!`, `msg_class!` and `msg_super!`
must be manually formatted at the moment.

### Comments

tapHLE does not use the `/* */` syntax for multiple-line comments, with the sole
exception of the "This Source Code Form is subject to […]" license header, which
should be the first comment in every file.

`//` syntax should be used for a short comment at the end of a line, a comment
that has its own line, or for one line out of many for a multiple-line comment.
Lines containing `//` comments must not exceed 80 characters, including
whitespace before the `//`. This rule is enforced by `dev-scripts/lint.sh`.

`/* */` can be used for tiny in-line comments to indicate the names of
parameters when calling a function, or to comment on their values. This is often
done for boolean parameters and others which don't have an obvious meaning
otherwise, e.g. `do_something(/* asynchronously: */ true)`.

### Naming things

There are many places in code where you have to give something a name:

```rust
fn foo_bar() {}         // function named foo_bar
const FOO_BAR: i32 = 1; // constant named FOO_BAR
struct FooBar {}        // struct named FooBar
// …
```

tapHLE has two main approaches to naming things:

* tapHLE implements many frameworks and libraries that can be used by the guest
  app (e.g. UIKit, OpenGL ES 1.1, the C standard library, and the Objective-C
  runtime). These frameworks/libraries have APIs and ABIs that define functions,
  constants, types, classes, methods and so on, and all these things have names.
  Let's call these "external names". Where possible and appropriate, tapHLE's
  implementations of these will have the same name as in the original API/ABI,
  and in their original forms (if the original is `kNSFooBar`, it won't be
  renamed to Rust-style `K_NS_FOO_BAR`, etc.).
* tapHLE internal code that is not _directly_ exposed to the guest app should
  use original names, to avoid [copyright
  concerns](#copyright-and-reverse-engineering), and generally follow the
  [naming conventions from the Rust API
  Guidelines](https://rust-lang.github.io/api-guidelines/naming.html). Let's
  call these "internal names".

It's not always clear which rule applies though, so here's some more specific
guidelines:

* Names that are part of the ABI must match external names exactly, otherwise
  tapHLE won't work. The main examples of these are the strings used in
  `FunctionExports`, `ConstantExports` and `ClassExports` lists, and Objective-C
  class names and selectors.
  * If you have no choice but to expose a _tapHLE-specific_ internal detail
    through the ABI, don't pick a name that looks like it might be external. Use
    a prefix like `_tapHLE_` to make it clear that the name originates from
    tapHLE, and to prevent potential conflicts with names coming from the guest
    app.
* Names of types and `#define`/enum-like constants (as opposed to `static
  const`-like/`ConstantExports` constants) are _not_ part of the ABI, but
  generally tapHLE will nonetheless use the API's name for it in its original
  form, in order to simplify cross-referencing of code with external
  documentation. For example, the type `UILineBreakMode` and its associated
  constants have the same names in [Apple's
  documentation](https://developer.apple.com/documentation/uikit/uilinebreakmode)
  and in [tapHLE's implementation](../src/frameworks/uikit/ui_font.rs). Note
  that due to [copyright concerns](#copyright-and-reverse-engineering) you must
  not copy the names of non-public API implementation details, including
  internal macros and types in C headers.
* When implementing a C/Objective-C function, the name of the Rust function
  (`fn some_function_name_here(env: &mut Environment, …)`) used to implement it
  is also not part of the ABI, but in almost all cases the `export_c_func!`
  macro is used to export it by the same name, so it's easiest to preserve the
  external name in its original form, and this is also the recommendation when
  this macro is not used. For example, `UIGraphicsPushContext` has the same name
  in [Apple's
  documentation](https://developer.apple.com/documentation/uikit/1623921-uigraphicspushcontext?language=objc)
  and in [tapHLE's implementation](../src/frameworks/uikit/ui_graphics.rs).
* The names of parameters for C functions and Objective-C methods (not to be
  confused with any relevant parts of the _selector_) are neither part of the
  API nor of the ABI, so in general the external names do not need to be
  preserved, and it might be unwise to mirror them too closely due to [copyright
  concerns](#copyright-and-reverse-engineering). They should follow Rust naming
  conventions.
* The names of Rust modules/files are a bit of an in-between. They are internal
  names but generally take some inspiration from the external names of the
  things they're for, so they follow the Rust module naming convention more or
  less (e.g. `UIKit` becomes `ui_kit`).

Note that preserving original forms may make the Rust compiler upset at you. Use
of `#[allow(non_camel_case_types)]`, `#[allow(non_upper_case_globals)]`,
`#[allow(non_snake_case)]` and `#[allow(clippy::upper_case_acronyms)]` where
necessary is encouraged.

## Internal documentation

Generate the Rust API documentation with:

```powershell
cargo doc --workspace --no-deps --open
```

## Troubleshooting the build

- If CMake cannot find Boost, confirm `vendor/boost/boost/version.hpp` exists.
- If CMake fails with "Compatibility with CMake < 3.5 has been removed", set
  `CMAKE_POLICY_VERSION_MINIMUM=3.5` — see the Windows prerequisites above.
- If Cargo reports missing native source, rerun `git submodule update --init
  --recursive`.
- If the executable cannot open fonts or dynamic libraries, run it from the
  repository root or copy all three runtime resources beside it.
- If an app fails, preserve the full tapHLE log and follow
  `docs/compatibility.md` before implementing unrelated framework stubs.
