# Max Adventure Free compatibility work note

- Branch and starting commit: `compat/max-adventure` from `f340f8d4`.
- Canonical artifact: `https://archive.org/details/app-id-233`,
  `MaxAdventureFree 1.2.ipa`; MD5 `c5fee449c86d3e693f6445a8f6e8c37f`, SHA-1
  `630c4639b663ae8ed477c4e37817e95298a06479`, SHA-256
  `043ea5c352a1ac151ff6002c0cc8510357243f699980a480a0d2e098c70ac610`,
  size 18,985,524 bytes. MD5/SHA-1/size match the live Archive metadata; the
  filename is Windows-safe as-is (no remap needed).
- Embedded identity (`tapHLE --info`): display name `Max Free`, bundle
  `com.imangi.maxadventurefree` (Imangi Studios), version `1.2`, minimum OS
  `3.0`, device family iPhone + iPad (universal). `Info.plist` declares
  `UISupportedInterfaceOrientations` = LandscapeRight, LandscapeLeft; status bar
  hidden.
- Availability check (2026-07-25): not re-confirmed against Apple lookup yet;
  do this before a report-worthy run.

## Highest milestone

None yet. The process launches and stays alive but does not present a frame.

## Proven facts (dirty worktree, 2026-07-25)

- The binary is **armv7** (`Loading armv7 slice for "MaxAdventureFree"`), which
  is the last log line emitted.
- After that line the process **blocks**: CPU time is flat (~0 ms over 5 s, so
  it is waiting on something, not busy-looping), no further log appears, and no
  EAGL renderbuffer or Core Animation frame is ever presented (frame capture
  times out).
- The block is **host-side, before any guest code runs.** The unconditional
  `echo!("CPU emulation begins now.")` at the top of the main-thread init
  coroutine (`src/environment.rs`) goes to stderr and **never prints**, so the
  guest CPU never starts. The stall is between the Mach-O slice load
  (`src/mach_o.rs`) and the first resume of the main-thread coroutine — i.e. in
  host-side dyld linking / `Environment` construction / main-loop startup, not
  in guest `+load`/static-init/`main`.
- The hang is **independent of device family** (same with the default iPhone and
  with `--device-family=ipad`) and of the `--landscape-*` orientation.
- No guest crash / panic / register dump — there is no fault PC to anchor on.

## Rejected hypotheses

- "Slow asset load, not a hang." Rejected: CPU is idle, so it is blocked, not
  doing silent work.
- "Wrong device family." Rejected: iPhone and iPad both block identically.

## Next discriminator

The stall is host-side and before guest execution, so `--gdb` (which debugs the
*guest*) will not help until the guest starts. Instead find where the native
Rust startup blocks between `Loading <arch> slice` and `CPU emulation begins
now`:
1. Add `echo!` trace markers along the load/link/Environment-construction path
   (bundle load → dyld bind → `Environment::new` → main-loop first resume) and
   rebuild; the last marker printed localizes the stall.
2. Or attach a native debugger (WinDbg / the VS debugger / `rust-lldb`) to the
   live `tapHLE.exe` and read the main thread's native stack — idle CPU means it
   is parked on a host wait (a lock, a channel `recv`, an SDL/window call, or a
   blocking file read).
Only after it presents a frame does orientation matter (landscape-only; may need
`--landscape-native`, see the tapHLE_default_options.txt entry pattern used for
Warlords HD).

## Checks run

- Artifact hash verification (MD5/SHA-1/size vs live metadata): pass; SHA-256
  recorded above.
