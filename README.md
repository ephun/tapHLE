# tapHLE

tapHLE is a high-level emulator for 32-bit iOS applications. Its mission is to
**make every 32-bit iOS game playable on modern mobile and desktop hardware.**

Instead of emulating an entire iPhone and operating system, tapHLE runs the
app's 32-bit ARM code and supplies its own implementations of frameworks such
as Foundation, UIKit, OpenGL ES, and OpenAL.

Open tapHLE, see your apps, pick one, press Play.

**tapHLE is experimental.** Compatibility is specific to an exact app version,
and many apps will not work yet. The project does not include apps, Apple
software, decryption keys, or other proprietary material.

**[See the compatibility ratings (1–5 stars)](https://taphle.ephun.net/compatibility)**

## Want a specific app to work?

You can use your own coding agent to work on it. You do not need to know how to
program, and you do not need to wait for someone else to take your request.
tapHLE gives the agent rules, tests, and a step-by-step debugging guide.

**[Start here: make an app work with a coding agent](docs/compatibility.md)**

Never upload an IPA, app files, or a raw log. Opening an issue does not promise
that another contributor will do the work; it gives you and your agent a place
to record the goal and avoid duplicate work.

## Project direction

This fork is AI-development-led. Coding agents are first-class contributors for
investigation, implementation, testing, and documentation.

Contribution is either driven by direct work on a specific app or by broad work
implementing iOS systems missing from the emulator. Broad work helps a lot of
apps a little, and specific app work helps one app a lot and maybe a couple
others unintentionally.

## Platforms

tapHLE targets Windows, macOS, Linux, Android and iOS, and is waiting to
release its first version until all five are ready.

Most work is done and tested on Windows first, and Windows is currently the
only host on which compatibility results are accepted.
**[docs/platforms.md](docs/platforms.md)** is the honest per-platform state —
what builds, what has actually been run, and what is packaged.

## Documentation

**[docs/README.md](docs/README.md)** is the map. The short version:

- **[Using tapHLE](docs/user-guide.md)** — running apps, settings, logs, saves
- **[Platform status](docs/platforms.md)** — what works where
- **[Developing](docs/development.md)** — build, test, contribute, code style
- **[Making an app work](docs/compatibility.md)** — the compatibility path
- **[Debugging](docs/debugging.md)** — diagnostic technique
- **[Architecture](docs/architecture.md)** — how tapHLE is put together
- **[Maintaining](docs/maintaining.md)** — releases, packaging, upstream

## Building

Install Git, Rust, CMake, a C/C++ toolchain, and Boost, then:

```powershell
git clone --recurse-submodules https://github.com/ephun/tapHLE.git
cd tapHLE
cargo build --release
```

That produces `tapHLE-gui.exe`, the desktop frontend, and `tapHLE.exe`, the
emulator it launches. Full prerequisites, troubleshooting and the test ladder
are in **[docs/development.md](docs/development.md)**.

## Contributing

If you want to use a coding agent for an app, start with
**[docs/compatibility.md](docs/compatibility.md)**. Agents must read
`AGENTS.md`. Human contributors can find more detail in `CONTRIBUTING.md`.

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

The project is not affiliated with or endorsed by Apple Inc. iPhone, iOS,
iPod, iPod touch, and iPad are Apple trademarks.

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
