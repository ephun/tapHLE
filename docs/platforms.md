# Platform status

This page is the only place tapHLE records what works on which host. Every
other document links here rather than keeping its own table, because five
copies of a status matrix is how the project ended up claiming Windows-only
releases and all-platform releases in the same file.

## What the words mean

A platform is not simply supported or unsupported. It occupies a position on
seven independent axes, and collapsing them is what produced sentences like
"Packaged: yes; installer not yet built."

| Term | Means |
| --- | --- |
| **Distribution target** | tapHLE intends to ship here. A statement of direction, not of state. |
| **Build verified** | Someone has compiled tapHLE for this host and the build succeeded. |
| **Runtime verified** | Someone has launched tapHLE on this host and watched a guest app run. Compiling is not running. |
| **CI** | A job in `.github/workflows/tapHLE_release.yml` exercises this host automatically. |
| **Package tooling** | A script exists that turns a build into something installable. Separate from whether anyone has run it. |
| **Release eligible** | A numbered release may ship an artifact for this host. |
| **Compatibility support** | The maintainer accepts compatibility results earned on this host. |

Two words are reserved and should not be used loosely anywhere in tapHLE's
documentation:

**Supported** means compatibility-supported — the maintainer accepts behavioral
and compatibility claims earned there. It never means "the code compiled once."

**Packaged** is not a yes/no. Say *bundle tooling*, *installer tooling*, or
*published artifact*, whichever you actually mean.

## Current state

All five are distribution targets: Windows, macOS, Linux, Android and iOS. The
maintainer set that bar on 2026-08-17, and **the first release ships on all
five platforms** — see [Release eligibility](#release-eligibility) below for
what that commits the project to.

Intent is not state. What is true today:

| Host | Distribution target | Build verified | Runtime verified | CI | Package tooling | Release eligible | Compatibility support |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Windows x86_64 | yes | yes | yes, continuously | yes | bundle yes; installer written, never built | pending the all-five bar | **canonical** |
| macOS x86_64 | yes | yes, in CI | no — nobody has played an app on it | yes | emulator-only bundle script, inherited | pending the all-five bar | no |
| Linux x86_64 | yes | attempted in CI, not confirmed | no | yes, `continue-on-error` | no | pending the all-five bar | no |
| Android | yes | no | no | no | no | pending the all-five bar | no |
| iOS | yes | branch only | no | no | no | pending the all-five bar | no |

### Windows x86_64

The primary development and compatibility environment. A compatibility result
is accepted here and nowhere else today, so in practice this is the host to
expect apps to work on.

`dev-scripts/make-windows-bundle.sh` assembles the redistributable directory
and runs in CI. `dev-scripts/tapHLE.iss` is a written, reviewed Inno Setup
script that **has never been built** — Inno Setup is not installed on the
development machine. Treat its first run as unproven.

### macOS x86_64

Builds in CI on `macos-15` with formatting, lint, unit and integration tests.
Nobody plays apps on it, so a macOS result is unverified by definition. It is
useful for comparing guest behaviour against Apple's own frameworks and for
debugging shared code.

`dev-scripts/make-macos-bundle.sh` is inherited from touchHLE, predates the
frontend, and bundles the emulator only.

### Linux x86_64

CI runs a `Linux x86_64 build` job on `ubuntu-latest` that installs the SDL2,
Wayland, X11 and GTK 3 dependencies, then runs `dev-scripts/lint.sh`,
`cargo test --workspace --lib` and `cargo build --release`, uploading both
binaries. It carries `continue-on-error: true` with an instruction to remove
that once it passes twice, so a green run has not yet been established as
routine — read "attempted in CI" literally and check the workflow's recent runs
before claiming Linux builds.

Nobody has run tapHLE on Linux. Write portable code; do not claim it works.

### Android

The source in `android/` is active work as of 2026-08-17, not inherited
material to leave alone. It has no frontend, which is the substantial piece.

### iOS

Work lives on `feat/ios-host`. An experimental host was merged to `trunk` on
2026-08-01 and withdrawn on 2026-08-04.

**Read why it was withdrawn before merging it back.** It was half-finished and
broken, and nothing on Windows could build or test it, so it sat on `trunk` as
untested code claiming a capability tapHLE did not have. That objection was
never about iOS being unwanted, and the 2026-08-17 direction does not answer
it — a branch is still the right home for a host nobody can run, and `trunk` is
still for what works. What changed is that making it runnable is now the job,
so the route back to `trunk` is to build and run it, not to relax the standard.

## Release eligibility

The first numbered release ships on all five platforms. Nothing is released
until every row above reaches "yes" in build, runtime and package tooling.
`dev-scripts/release-readiness.ps1` reports this as a blocker so the rule is
visibly held rather than quietly not working.

Three requirements follow, and they are requirements rather than aspirations:

- **A shared version number has to be earned.** Platforms may carry the same
  version only where testing has independently established that an app rated
  three stars on one platform is three stars on the others. Where that has not
  been established, the platforms get different version numbers. The rule
  exists so that one version number never implies compatibility nobody
  measured.
- **The mobile frontends are in scope, not a later port.** Each needs the
  report-submission options, whichever settings apply on that platform, and the
  help/about sections — the same product, not a cut-down viewer.
- **The design is one design.** Mobile and all three desktops must read as two
  sides of the same coin, with consistent branding across the five. A frontend
  that works but looks like a different project does not meet the bar.

None of this lowers the evidence standard; it raises the amount of evidence
owed. "Builds on a platform" is still not "works on a platform", and a release
claim for a platform still needs somebody to have run it there.

## Compatibility results are host-qualified

A compatibility result records the host it was earned on, and says nothing
about any other host. A three-star result on Windows does not become a
three-star result on macOS because the code is shared; somebody has to run it
there. This is the same rule as the shared version number above, applied to a
single app instead of a release.

`docs/compatibility.md` has the full protocol.

## What each platform still needs

`docs/maintaining.md` records the packaging work outstanding per host —
what a Linux attempt would run into, what a frontend-carrying macOS bundle
requires, and why neither mobile platform has a frontend yet.
