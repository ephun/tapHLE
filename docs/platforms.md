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

The rest of tapHLE's controlled vocabulary — app versus game, host versus
guest, numbered release versus development build — is in
[`docs/README.md`](README.md#words-taphle-uses-precisely).

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
| Linux x86_64 | yes | attempted in CI, not confirmed | no | yes, `continue-on-error` | portable bundle script, real build pending | pending the all-five bar | no |
| Android | yes | yes | yes — synthetic app launch, touch and library return | no | debug APK tooling | pending the all-five bar | no |
| iOS | yes | yes — Intel simulator | partial — frontend visible; synthetic guest UI incomplete | no | Xcode app-bundle tooling; no signed IPA | pending the all-five bar | no |

### Windows x86_64

The primary development and compatibility environment. A compatibility result
is accepted here and nowhere else today, so in practice this is the host to
expect apps to work on.

`platforms/windows/make-bundle.sh` assembles the redistributable directory
and runs in CI. `platforms/windows/installer.iss` is a written, reviewed Inno Setup
script that **has never been built** — Inno Setup is not installed on the
development machine. Treat its first run as unproven.

### macOS x86_64

Builds in CI on `macos-15` with formatting, lint, unit and integration tests.
Nobody plays apps on it, so a macOS result is unverified by definition. It is
useful for comparing guest behaviour against Apple's own frameworks and for
debugging shared code.

`platforms/macos/make-bundle.sh` is inherited from touchHLE, predates the
frontend, and bundles the emulator only.

### Linux x86_64

CI runs a `Linux x86_64 build` job on `ubuntu-latest` that installs the SDL2,
Wayland, X11 and GTK 3 dependencies, then runs `dev-scripts/lint.sh`,
`cargo test --workspace --lib` and `cargo build --release`, then uploads the
portable bundle. It carries `continue-on-error: true` with an instruction to remove
that once it passes twice, so a green run has not yet been established as
routine — read "attempted in CI" literally and check the workflow's recent runs
before claiming Linux builds.

Nobody has run tapHLE on Linux. Write portable code; do not claim it works.

### Android

`platforms/android/` builds an APK whose SDL activity loads `tapHLE_gui`. The
library, settings, emulator and guest-app runtime are the same Rust components
used by the desktop product; only the Android bootstrap, document provider and
packaging are platform code. The development APK has been run visibly in
Cuttlefish on x86_64: the adaptive frontend rendered in portrait and landscape,
the synthetic TestApp rendered and accepted touch input, and Android Back ended
the guest run and returned to the library. CI and release packaging remain
outstanding.

### iOS

`platforms/ios/` is a thin Objective-C/SDL process bootstrap and Xcode host for
the same `tapHLE_gui` Rust library. Xcode 16.4 on Intel macOS builds an x86_64
iOS 18.5 simulator app containing the guest libraries, fonts and default
options. The shared frontend has been inspected in the real Simulator session
in portrait and landscape, including safe-area handling, and it can launch the
synthetic TestApp into the emulator.

This is not yet a complete guest runtime result. The iOS simulator needs the
OpenAL Soft null backend when its virtual audio device is unavailable, and the
synthetic guest currently reaches CPU emulation and a presented frame but does
not render its UIKit controls or visibly react to touch. No physical-device,
signed IPA or return-to-library result has been earned. Keep those gaps visible;
a simulator frontend is not a release-eligible iOS product.

## Release eligibility

The first numbered release is `0.2.4` and ships on all five hosts. Nothing is
released or tagged until every row above reaches yes for build, visible runtime
and installable package tooling, and the frozen app matrix is complete.
`dev-scripts/release-readiness.ps1` must keep this gate visible.

The `0.2.4` cohort froze on 2026-08-24. Its exact 24 app versions are the records
in `compatibility/release-cohorts/0.2.4.json`; do not add or remove an app after
the freeze because a later database rating changed. Every one of those app
versions must independently reach at least three stars on Windows, Linux, macOS,
Android and iOS.

All five products for a candidate must be built from one full Git commit. Each
verification records the host, architecture, OS version, full commit, binary or
package SHA-256, build provenance and profile, exact app artifact identity,
rating/frontier, producer identity and verification type. The commit is the
canonical source identity; a package hash proves which output was actually run.

A development-candidate matrix is followed by the final changelog and release
identity commit. Because that changes the source and package hashes, the final
five-host matrix must be rerun on the exact final commit before it is tagged.
Evidence from the development candidate is a discriminator, not release evidence
for the later commit.

Three requirements follow:

- **One version is earned by one complete matrix.** A shared version never
  implies compatibility copied from another host.
- **The mobile products are real products, not later ports.** Android and iOS
  must build, install and run visibly on their assigned devices. Each includes
  the library, applicable settings, and help/about surfaces. Reporting is shown
  only when its secure authentication workflow is complete.
- **The design is one design.** Mobile and all three desktops carry consistent
  tapHLE branding and product identity.

A successful build is not a runtime result. GUI claims require the target's real
visible session and inspected window evidence; a background or SSH-only process
is not visible-runtime evidence.

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
