# tapHLE

tapHLE is a high-level emulator for early iPhone OS applications. Its goal is
to make as many early iPhone games as possible run well on the desktop.

Open tapHLE, see your apps, pick one, press Play.

## Want a game to work?

You can use your own coding agent to work on it. You do not need to know how to
program, and you do not need to wait for someone else to take your request.
tapHLE gives the agent rules, tests, and a step-by-step debugging guide.

**[Start here: make a game work with a coding agent](HELP_A_GAME.md)**

Opening an issue does not promise that another contributor will do the work.
It gives you and your agent a place to record the goal and avoid duplicate
work. Never upload an IPA, game files, or a raw log.

Instead of emulating an entire iPhone and operating system, tapHLE runs the
game's 32-bit ARM code and supplies its own implementations of frameworks such
as Foundation, UIKit, OpenGL ES, and OpenAL.

## Project direction

This fork is AI-development-led. Coding agents are first-class contributors
for investigation, implementation, testing, and documentation. Contributors
may choose the games they care about. Each game is a practical step toward
broader compatibility.

The priorities are intentionally practical:

1. Move one chosen game closer to working on its requested supported host.
2. Iterate quickly from logs and observed behavior.
3. Protect working behavior with focused tests.
4. Improve architecture when it helps deliver compatibility.

Bounded compatibility workarounds are acceptable. Elegance is welcome, but a
large from-the-ground-up implementation is not a prerequisite for a useful
fix.

## Platforms

The desktop — Windows, Linux and macOS — is the priority. iOS and Android are
intended eventually and are not being worked on now.

That is a statement of intent, so here is the state of each one separately.
Nothing in this table is a promise about the next row along.

| Platform | Builds | Tested | Packaged |
| --- | --- | --- | --- |
| Windows x86_64 | yes | yes, continuously | yes; installer scripted but not yet built |
| macOS x86_64 | yes | built in CI, not played on | bundle script, emulator only |
| Linux x86_64 | not tried yet | no | no |
| iOS | on the `feat/ios-host` branch | no | no |
| Android | inherited source only | no | no |

Windows is where tapHLE is developed and where compatibility is judged, so it
is the platform to expect things to work on. macOS compiles and is useful for
comparing behaviour against Apple's own frameworks. Linux has not been tried;
nothing in the code is Windows-specific by design, and `dev-docs/packaging.md`
lists what a first attempt would run into.

Running on modern iOS is a likely future direction rather than something tapHLE
does today. An experimental host was tried and withdrawn from `trunk` because it
was unfinished and could not be built or tested; that work continues on the
`feat/ios-host` branch. The inherited Android source remains in the repository
and is not being developed.

## Status

tapHLE is experimental. Compatibility is specific to an exact game version,
and many applications will not work yet. The project does not include games,
Apple software, decryption keys, or other proprietary material.

The first tapHLE release line is `0.3`; numbered alpha/beta/RC/stable builds are
Windows x86_64 releases, while green `trunk` builds are commit-identified
previews. **No release has been published yet**, so tapHLE's update check
correctly reports that there is nothing to compare against. The versioning and
packaging policy is documented in `dev-docs/releases.md`.

**[See the compatibility ratings (1–5 stars)](https://taphle.ephun.net/compatibility).**
That live database is the current record; the legacy JSON records remain only
until they are migrated. Every result names the exact app build it was earned
on, read from the bundle metadata of the file that was actually run, and a
record for a game that is no longer sold may also note where that file came
from. Those rules do not cover a game that is still sold, and the project
respects DMCA notices and rightsholder requests. `compatibility/README.md` is
the full protocol.

The project is not affiliated with or endorsed by Apple Inc. iPhone, iOS,
iPod, iPod touch, and iPad are Apple trademarks.

## Build on Windows

The full prerequisites and troubleshooting notes are in
`dev-docs/building.md`. At a high level, install Git, Rust, CMake, a C/C++
toolchain, and Boost, then run:

```powershell
git clone --recurse-submodules https://github.com/ephun/tapHLE.git
cd tapHLE
cargo build --release
```

That produces two programs in `target\release`:

- `tapHLE-gui.exe` — the desktop frontend: your app library, settings and an
  integrated log.
- `tapHLE.exe` — the emulator itself, which the frontend launches and which
  you can also run directly from a terminal.

A distributable directory also needs `tapHLE_dylibs`, `tapHLE_fonts`,
`res\icon.png` and `tapHLE_default_options.txt`; `dev-scripts/make-windows-bundle.sh`
assembles those, and `dev-docs/packaging.md` covers the installer.

## Using it

Put `.ipa` files or `.app` bundles in `tapHLE_apps`, or drop them onto the
window, or use **Add App**. Those files are ignored by Git, so you can keep
playtest targets there without committing or redistributing them.

```powershell
.\target\release\tapHLE-gui.exe
```

Select an app and press **Play**. It opens in its own window; the library stays
open, so you can close the app and start another without restarting anything.

**Settings** are global, and any app can override any of them from its own
**Settings…** button. Anything you do not override follows the global default,
then `tapHLE_options.txt`, then the per-app entries in
`tapHLE_default_options.txt` that make particular games work.

**View ▸ Log / Output** opens a panel along the bottom carrying the emulator's
own output, with severity, subsystem and text filters. It is hidden by default
and keeps recording while hidden, so it is worth opening after something goes
wrong as well as before. If an app stops unexpectedly, tapHLE says so and
offers the log rather than letting its window vanish.

The frontend keeps its library, settings and window state in `tapHLE_frontend`,
as readable JSON. Guest save data is in `tapHLE_sandbox`. Neither is touched by
an uninstall.

### From the command line

The emulator is unchanged and still runs on its own:

```powershell
.\target\release\tapHLE.exe "C:\path\to\Game.ipa" --landscape-native
```

An app path is required: picking one from a library is the frontend's job.
`--help` lists every option, `OPTIONS_HELP.txt` explains them, and `--info`
prints what an app says about itself without running it. The frontend passes exactly these options, so
anything it does can be reproduced by hand — and `tapHLE_options.txt` applies
to both.

The version and numbered release rules are in `dev-docs/releases.md`.

## Contributing

If you want to use a coding agent for a game, start with `HELP_A_GAME.md`.
Agents must read `AGENTS.md`. Human contributors can find more detail in
`CONTRIBUTING.md`.

Game compatibility reports are especially useful when they identify the exact
game version, supported host environment, reproduction steps, and sanitized
tapHLE log. Never attach an app binary or raw log.

## Origin and license

tapHLE is a fork of the
[touchHLE project](https://github.com/touchHLE/touchHLE). Upstream deserves
credit for the emulator architecture and the substantial implementation this
fork began with; tapHLE has independent goals and contribution policies.

tapHLE modifications are copyright their respective contributors. The
inherited code remains copyright the touchHLE project contributors and other
authors identified in the source and bundled notices.

The emulator source is licensed under the Mozilla Public License 2.0. Due to
dependency license compatibility, distributed binaries are licensed under the
GNU General Public License version 3 or later. Bundled dynamic libraries and
fonts have their own notices in `tapHLE_dylibs` and `tapHLE_fonts`.

## A note on AI from the maintainer
I would specifically like to thank @hikari_no_yume and @ciciplusplus for their
work on the touchHLE project. Their passion in preserving things that would
otherwise be lost to time is truly beautiful. Additionally, thanks to
@johnny901901901 for laying the groundwork for an implementation of the emulator
on modern iOS (https://github.com/johnny901901901/touchHLE). Other inspiration 
comes from the LiveExec32 experimentation by the LiveContainer team 
(https://github.com/LiveContainer/LiveExec32). Their work and the human programmatic touch
required is truly indispensable. 

I fully recognize that there is controversy surrounding AI-generated code, and 
in my case, what would probably be most accurately described as "vibe-coding". 
While I am a decently tech-savvy person and know a little bit of coding, I would 
never describe myself as a programmer. I use AI to implement fixes that I, at the
end of the day, do not understand. 

Ethically (whether it be the environment, the security of the program, problems 
with AI training data, concerns about AI's impact on the job market in computer 
science), I do get it. I'm often on the fence about it myself, and I understand 
someone who has spent their life honing a skill like coding might have valid 
anger seeing someone throw a project together without a deep understanding of the 
mechanisms that make it possible. I very well may abandon this project, because
as data centers get built in my community, I feel less and less comfortable heavily
utilizing AI. In theory, I think it's one of the coolest advancements in technology
ever, but I never want it to come at the cost of human ingenuity or humanity in art.
Feel free to reach out to me and tell me your thoughts. I'm all ears. -@ephun
