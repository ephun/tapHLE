# Debugging tapHLE

Techniques for once you have localized a failure. The process around them — what
to investigate, what a claim means, how to record a result — is in
`docs/compatibility.md`. How the pieces fit together is in
`docs/architecture.md`.

Most of what follows is written for Windows, because that is where tapHLE is
developed. The general methods are host-independent; the harness sections are
not.

## Logging

`src/log.rs` provides two logging macros, `log!()` and `log_dbg!()`. The former
always prints; the latter only prints if the containing module is listed in
`ENABLED_MODULES` in the same file.

Modules worth enabling:

- `tapHLE::abi` and `tapHLE::dyld` together give a trace of almost all
  guest-to-host calls.
- `tapHLE::mem` logs memory allocations and deallocations.

## Instrument one boundary at a time

Temporary diagnostics should answer one named question and produce little output.
Gate them by the exact format, API, queue, or bundle when practical.

Good diagnostic fields include:

- an API's input structure and status code;
- buffer byte size, count, pointer route, and a four-byte signature;
- the first few packet descriptors or offsets;
- one allocation size/address and its allocate/free/reuse events; or
- one UI phase, coordinate, and resulting level/screen transition.

Do not dump entire audio buffers, textures, guest memory, app binaries, or raw
logs. Never commit a diagnostic containing proprietary bytes or personal paths.
After the question is answered, remove trace-only code or convert the small
useful failure message into a normal diagnostic.

Read structures before reinterpreting data. For compressed audio, capture the
`AudioStreamBasicDescription`, packet-description route/count, and the first sync
bytes before treating the buffer as PCM. For a stale-pointer crash, prove
allocation/free/reuse behavior before disabling an allocator rule globally.

Use existing dependencies and subsystem abstractions before introducing a new
decoder, parser, or host library.

## Crashes in host code

`RUST_BACKTRACE=1` is always helpful. Use a debug (not `--release`) build for the
best output.

## Crashes in guest code

tapHLE prints the basic registers (r0-r13, SP, LR, PC) and a frame-pointer stack
trace for the current thread when a panic occurs. Preserve that small diagnostic
in normal builds: it can turn a null memory error into a single disassembly
lookup without a trace rebuild.

To make sense of the result you will probably want the app binary open in Ghidra
or another reverse-engineering tool.

### An opaque MemoryError is usually a name you already have

`Error during CPU execution: MemoryError` is what tapHLE reports for *any* bad
guest access, so it names nothing by itself. Two cheap steps turn it into a
symbol, and the second is nearly free.

**Try this first — it needs no tools at all.** tapHLE already logs, for every
symbol it could not bind:

```text
Warning: unhandled non-lazy symbol "_kCFAbsoluteTimeIntervalSince1970" at 0x433e8
```

That address is the same one the faulting instruction loads from. These warnings
are common and usually filtered out as noise. **They are only harmless while the
guest does not dereference them.** An unbound `__nl_symbol_ptr` slot is a null
pointer sitting in `__DATA` waiting for someone to read it, and the guest reads
it without checking. So when a `MemoryError` appears, grep the log for the
address the fault touched before doing anything else.

Exporting one such constant took JellyCar 1 from crashing during startup to its
full main menu and into level loading.

**Then disassemble the faulting PC.** `dev-scripts/disasm-guest-fault.py
<app.ipa> <pc>` takes the PC straight from tapHLE's register dump and prints the
instructions around it, resolving the guest address through the Mach-O segments
and handling fat binaries and Thumb.

JellyCar 1 faulted at `0x30190`:

```text
0x0003018e  ldr   r3, [r3]        <- r3 = *(a global)
0x00030190  vldr  d7, [r3]        <== FAULT
```

with `R3 = 0` in the dump. So a global pointer was null and the code loaded a
double through it.

If the faulting instruction dereferences a zero register, inspect Mach-O non-lazy
data imports as well as lazy function imports. Compiler support such as
`___stack_chk_guard` is data, so a missing relocation may survive startup and
only crash later when generated code loads through the unresolved slot.

### Disassembling by hand

On Windows, check the LLVM tools already installed with the pinned Rust toolchain
before installing another disassembler. After extracting only the authorized app
executable to a unique temporary directory:

```powershell
$llvmBin = Join-Path (rustc --print sysroot) `
    'lib\rustlib\x86_64-pc-windows-msvc\bin'
$objdump = Join-Path $llvmBin 'llvm-objdump.exe'
$appBinary = '<temporary path to the extracted app executable>'
$faultPc = '1234' # clear the Thumb bit when an address includes it

& $objdump --macho --arch=armv6 --disassemble $appBinary |
    Select-String -Pattern "^\s*$faultPc`:" -Context 40,60
```

Use `llvm-nm.exe --arch=armv6 --defined-only` from the same directory to map
nearby methods and C++ functions. Keep only the small address context needed to
explain the fault; do not save or commit a full disassembly. Reconcile the
faulting operands with tapHLE's register dump.

For an Objective-C++ object, also check compiler-emitted `.cxx_construct` and
`.cxx_destruct` methods before treating a zeroed C++ container as valid
initialized state.

Resolve the program counter to its owning app image, library, or HLE boundary
before designing a fix. A native library fault, managed-code fault, and emulator
panic have different recovery paths.

### Recognize compiler-created Objective-C blocks

Clang emits stack and global block literals directly in guest memory. They are
Objective-C-compatible objects, but they were not allocated through tapHLE's
normal object table. A high stack address receiving `copy`, combined with an
unhandled `__NSConcreteStackBlock` or `__NSConcreteGlobalBlock` relocation, is
evidence of a missing Blocks runtime rather than a corrupt ordinary object.

Trace only the final selector sequence and inspect the stable block prefix:
`isa`, flags, reserved word, invoke pointer and descriptor pointer. Implement the
documented Blocks ABI coherently: external block classes, stack-to-heap copy by
descriptor size, copy/dispose helpers, heap reference counts,
`_Block_object_assign`, `_Block_object_dispose`, and `__block` forwarding.
Returning the original stack pointer from `copy` may move one run forward but
leaves a use-after-return bug, so it is not a valid compatibility fix.

### Suspect the emulator's own iteration before the app's ownership

A message arriving at an object whose class differs from run to run is a
use-after-free. The tempting next step is to hunt for a retain the app is
missing, but check the other side first: does tapHLE deliver that message by
iterating a **copy** of a callback list it took before running guest code?

`NSNotificationCenter`, responder and delegate lists, timer and run-loop queues
all have to copy before dispatching, because delivery reenters the guest and the
guest may register or unregister during it. Copying is correct; iterating the
copy blindly is not. These lists deliberately hold unretained references, so an
entry that unregisters and is deallocated part-way through a dispatch leaves the
copy naming freed memory. Cocoa's guarantee that a removed observer is not
messaged is exactly what makes unregistering in a teardown method safe, so a
faithful implementation has to re-check each copied entry against the live list
before messaging it.

Two traps make this easy to misdiagnose:

- Instrumenting `retain`/`release` and attributing each call by whether a guest
  `LR` is available will label the emulator's own retain around a dispatch as
  guest code. A retain/release pair that brackets exactly one send, and nothing
  else, is more likely to be the host's dispatch guard than an app's.
- If the app routes its own event system through a Foundation class, the guest
  method names will not say so. Resolve the wrapper: a two-call method that
  fetches a singleton and then sends one selector to it is almost always a thin
  forwarder, and resolving its `__objc_selrefs` entries names the real API.

## Dumping classes, selectors and symbols

`--dump=linking-info` dumps the classes, selectors, and lazy symbols requested by
the binary, and how tapHLE is handling them. Output goes to the file given by
`--dump-file=` (default `DUMP.txt` in the running directory).

The most useful application is determining which classes, selectors or functions
an app might need that tapHLE does not implement. Check with
`dev-scripts/log-unimplemented.sh [name of app to check]` — make sure `jq` is
installed.

The JSON schemas are described in `ObjC::dump_classes` (`src/objc/classes.rs`),
`ObjC::dump_selectors` (`src/objc/selectors.rs`), and `Dyld::dump_lazy_symbols`
(`src/dyld.rs`).

## Before implementing a stub, check whether tapHLE already ships the real thing

`tapHLE_dylibs/` holds real Apple-era libraries — `libgcc_s.1.dylib`,
`libstdc++.6.0.9.dylib`, `libxml2`, `libz`, `libsqlite3` — and an app that links
one of them gets the genuine implementation loaded as guest code. So when an app
stops inside a tapHLE stub, the question is not only "how do I implement this",
it is **"is this already here?"**

It was, for the whole SjLj unwinder. tapHLE kept the function-context chain and
stopped at a throw with "SjLj unwinding is not implemented", which reads like a
missing feature and would have cost a large, delicate piece of work to write: the
personality routine, the LSDA tables, installing a context. `llvm-nm` on the
bundled `libgcc_s.1.dylib` shows `__Unwind_SjLj_RaiseException` and the rest of
the family defined, right next to the two calls tapHLE was stubbing. Both
versions of Bookworm went from dying during start-up to running, with no unwinder
written at all.

Two checks, both cheap:

```powershell
$llvmBin = Join-Path (rustc --print sysroot) `
    'lib\rustlib\x86_64-pc-windows-msvc\bin'
& (Join-Path $llvmBin 'llvm-nm.exe') --defined-only tapHLE_dylibs\libgcc_s.1.dylib |
    Select-String Unwind
```

and, on the app, whether it links the library at all — a `LC_LOAD_DYLIB` for
`/usr/lib/libgcc_s.1.dylib` is what decides whether the real implementation is
even present for that app.

**Which one binds is not obvious, and was not consistent.** tapHLE's host exports
normally win, which is right: they are the ones that know about the emulator. But
a *set* of functions sharing hidden state has to come from one place, and the two
binding paths disagreed about which — a non-lazy symbol pointer already preferred
a guest dylib's definition, while a lazy stub preferred the host's.
`guest_definition_wins` in `src/dyld.rs` is where that exception is stated;
extend it only for the same shape of problem, and say why.

## The GDB remote serial protocol server

For more complex cases, `--gdb=` starts tapHLE in debugging mode, providing a GDB
Remote Serial Protocol server. (In theory LLDB also should work, but it doesn't.)

This will not be the GDB experience you may be used to from C/C++ in debug mode.
GDB support was added to help debug apps for which there are no symbols, let
alone DWARF info or source. GDB connected to tapHLE will not know about local
variables or even stack frames — you need instruction addresses and register
numbers. Having the binary open in Ghidra is practically mandatory.

You need a GDB that supports ARMv6. On Windows, use an ARM-capable GDB from a
cross-toolchain and verify that its architecture list includes ARM; an x86-only
GDB cannot debug the guest. On macOS, the Homebrew `gdb` package is
multi-architecture. On Ubuntu, `gdb-multiarch` may work.

```sh
tapHLE --gdb=localhost:9001 'Some App.app'
gdb 'Some App.app/SomeApp' -ex 'target remote localhost:9001'
```

Omitting the executable path leaves GDB with no debug symbol info, [which may be
a worse experience](https://sourceware.org/bugzilla/show_bug.cgi?id=30234).

When GDB first connects, CPU execution is paused and none of the guest app's code
has run yet. While paused, tapHLE allows GDB to read and write registers, read
and write memory, resume execution indefinitely or for a single instruction, and
kill the emulated app (this just makes tapHLE crash).

Useful commands: `break *0x1000`, `info registers`, `backtrace` (though tapHLE's
own may be better), `print *(float*)0x2000`, `layout asm`, `step`, `continue`.

iPhone OS apps often contain a mix of Thumb and Arm functions, and GDB usually
won't know which it is dealing with:

- With no symbols, GDB assumes Arm by default. `set arm fallback-mode` changes
  that assumption.
- With full symbols, GDB seems to assume symbols are for Arm functions [even when
  they aren't](https://sourceware.org/bugzilla/show_bug.cgi?id=30386). `set arm
  force-mode` overrides it.

GDB [mostly](https://sourceware.org/bugzilla/show_bug.cgi?id=30385) understands
the convention of setting the low address bit to 1 for Thumb, and setting an Arm
breakpoint in Thumb code (not vice-versa) usually works, so this mainly matters
when disassembling.

tapHLE only communicates with GDB while execution is paused — on connect, on
certain CPU errors, and after stepping. Breakpoints force a pause at a convenient
location. Pressing F12 with the tapHLE window focused makes tapHLE pause during
the next `NSRunLoop` iteration; if the app fails to return to the run loop that
won't help.

## Graphics debugging

[apitrace](https://apitrace.github.io/) is invaluable for OpenGL issues.

More generally, and especially outside the OpenGL realm, sometimes the most
effective solution is dumping image data to a file. There are functions in
[`crate::debug`](../src/debug.rs) for this, and `std::fs::write` works too. GIMP
and some other tools can read raw pixel data, easiest if the filename ends in
`.data`.

Text and anything else with a handedness can be turned over twice on its way to
the screen. `docs/architecture.md` explains both flips and why keying on the
transform alone is wrong.

When the fault is an assertion at an HLE graphics boundary, recover the exact
guest API call and enum before relaxing it. Some old apps make invalid GL calls
that a real driver answers by recording `GL_INVALID_ENUM` and continuing. A
bounded emulator fix should preserve that guest-visible error behavior instead of
converting the app mistake into a host panic or ignoring all GL errors.

## Layout and drawing are separate boundaries

For an archived UIKit screen, diagnose drawing and hit testing separately.
`UISubviews` is stored back-to-front, so a decorative full-screen view near the
end can intercept every control even when the buttons render correctly. Check the
exact archived interaction flag and any content wrapper objects (for example,
image-only button content) before changing event routing or drawing. When a
missing keyed value and an explicit false value have different semantics, use
`containsValueForKey:` before decoding; otherwise a decoder can silently replace
a subclass-specific default.

A view created after launch can be mounted, touchable, and still blank if its
custom `layoutSubviews` never runs. For an EAGL-backed view, distinguish these
checkpoints in order: display-link creation, layout-driven drawable allocation, a
nonzero bound renderbuffer, and `presentRenderbuffer:`.

Do not paper over a missing layout pass by calling every view's layout method
every frame; that can make an app destroy and recreate its framebuffer
continuously. `docs/architecture.md` records the current layout-on-mount
behaviour and its known gaps.

## Running on Windows: making runs isolated and repeatable

Use a uniquely named temporary directory as tapHLE's working directory. Link its
`tapHLE_dylibs` and `tapHLE_fonts` to the checkout and copy the small tracked
default-options file rather than copying large support trees. Do not add local
options unless the experiment is specifically testing one.

Do not use `--headless` merely to hide an automated run. UIKit and EAGL paths can
legitimately require `Environment.window`; headless mode may create an earlier,
unrelated unwrap or no-context failure. Use it only when the named test is
genuinely window-independent.

For each meaningful run, record:

- exact tapHLE commit, or that the build was dirty;
- verified IPA hash;
- tapHLE arguments and options source;
- client-area input coordinates and timing;
- last screen/event reached and whether the process stayed alive; and
- the narrow log lines supporting the conclusion.

### A tap that does nothing is usually the harness, not the app

On a display with scaling — the development machine runs at 175% — tapHLE's
window is DPI-unaware, so Windows virtualises its coordinates. A harness that is
itself DPI-aware and converts client coordinates with `ClientToScreen` therefore
puts the cursor well away from the button it aimed at, and every control in every
app looks unresponsive.

That cost four apps a wrong conclusion in one session on 2026-08-16, and the
wrong conclusion was convincing: the touch really was delivered, the app's
`hitTest:` and `pointInside:` really did run, and they honestly answered "not in
this button". One of those apps went from "its buttons do not respond" to three
stars on the strength of the coordinate change alone.

So: **measure the target on a screen capture and click at that offset from the
window's origin.** Then diff the frames either side of the tap; a tap that changed
nothing is a fact about the pixels, not yet a fact about the app. If several apps
in a row appear to ignore taps, suspect the harness first.

A clickmap records *client* coordinates by design, so its numbers have to be
converted before a harness of this kind can use them.

Set `PER_MONITOR_AWARE_V2` before capturing, or screenshots are silently cropped
and look like layout bugs.

**Verify the window is under the point before clicking.** A stray synthetic click
lands on whatever is actually there.

### Foreground input

Use real foreground mouse input when Windows message injection does not reach
SDL. Before every injected event, re-check that the foreground window belongs to
the exact spawned tapHLE process; abort input if focus or process identity
changed. Save and restore the previous cursor position and foreground window, and
stop only the exact process launched by the run.

If Windows foreground locking rejects `SetForegroundWindow`, temporarily attach
the harness thread's input queue to both the thread that currently owns the
foreground and the verified tapHLE window thread. Re-read both handles and thread
IDs on every bounded retry, detach in reverse order before sleeping, and verify
the exact target handle again after positioning the cursor and before injecting
input. **Do not send a synthetic Alt key while an unrelated app owns the
foreground**; it can alter that app's state. Abort safely if dual attachment
cannot establish ownership.

Allow the target event loop to consume the cursor move before sending the
button-down event — a short bounded pause such as 200 ms is sufficient in the
current harness. When touch routing is the boundary under test, confirm the trace
contains a `MouseMotion` at the intended client coordinate before the matching
`MouseButtonDown`. Sending the press immediately after `SetCursorPos` can make
SDL use the previous cursor coordinate and create a false hit-testing diagnosis.

Verify that `FocusGained` precedes the first intended client click. Windows may
consume the first click only to activate the emulator even when
`SetForegroundWindow` was requested. A reliable harness can click the spawned
window's title bar, wait for the focus event, and only then begin the recorded
client-coordinate recipe. Never use a focus click inside the app as if it were
part of the compatibility test.

### Screen capture, not PrintWindow, for anything drawn with OpenGL

`PrintWindow` copies a window's GDI surface. An accelerated OpenGL window has
nothing there, so it can come back black while the app is plainly visible. Copy
from the screen instead when the result matters, and treat a black `PrintWindow`
capture of a GL app as no evidence at all.

Desktop capture grabs whatever is on top. An occluding window reads as an app
rendering fault — check window occupancy before believing a screenshot.

### The Antigravity CLI interactive-desktop harness

This harness is for **Google Antigravity CLI (AGY) only**. Codex and other agent
surfaces must not use `agy-visible-taphle.ps1`; they should use their own
visible-window launch, inspection, input, and shutdown facilities instead.

AGY's ordinary Windows commands run on a background desktop. Direct execution,
`Start-Process`, and `cmd /c start` can initialize tapHLE's SDL, OpenGL, and
audio paths while leaving its window invisible to the logged-in user. AGY must
use the repository harness for all GUI-sensitive operations.

Run exactly one step at a time and stop on any nonzero exit:

```powershell
cargo build --release

powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Launch `
    -AppPath '.\tapHLE_apps\<exact verified filename>.ipa'

powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Status

powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Capture

powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Focus
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Click -X 384 -Y 512

powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Capture

powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Close
```

Never guess an Archive filename; use a literal, verified path.

The harness creates per-user Scheduled Tasks with `Interactive` logon type. The
launch task keeps tapHLE on the logged-in desktop and configures tapHLE's
internal frame capture. The short-lived input task focuses, clicks, or posts
`WM_CLOSE` on that same desktop. State and logs live outside the checkout under
`%LOCALAPPDATA%\tapHLE\agy-visible`.

A click-map step is proven only when `Status` returns a nonzero window handle,
the pre-click frame visibly identifies the expected starting screen, `Click`
succeeds, and the post-click frame visibly identifies the resulting screen.
Audio, a zero command exit, or a running background task is not visual proof. Use
`-Action Uninstall` only to intentionally remove the scheduled tasks.

A GUI launched through a POSIX shell tool may be invisible on the maintainer's
desktop; launch from PowerShell.

## Frame capture

Windows desktop screenshot APIs may return a black OpenGL client area. tapHLE can
capture the next rendered frame after an explicit request marker appears. EAGL
apps capture the next valid renderbuffer submitted through
`presentRenderbuffer:`. UIKit-only screens capture the next frame presented by
the Core Animation compositor. This is a harness feature, not an app option: set
both paths in the child process environment before launch, then create the
request marker only after the target state is reached.

```powershell
$captureDir = Join-Path ([IO.Path]::GetTempPath()) (
    'taphle-frame-' + [Guid]::NewGuid().ToString('N')
)
[void](New-Item -ItemType Directory -Path $captureDir)
$tapHLEPath = '<absolute path to the exact committed tapHLE.exe>'
$ipaPath = '<absolute path to the exact verified IPA>'
$requestPath = Join-Path $captureDir 'frame.request'
$outputPath = Join-Path $captureDir 'frame.ppm'

$env:TAPHLE_FRAME_CAPTURE_REQUEST = $requestPath
$env:TAPHLE_FRAME_CAPTURE_OUTPUT = $outputPath
try {
    # Start-Process returns immediately, which lets the harness create the
    # marker while this exact child is running. Set up the capture directory's
    # resource links/default-options file as described earlier in this section.
    $process = Start-Process `
        -FilePath $tapHLEPath `
        -ArgumentList ('"' + $ipaPath + '"') `
        -WorkingDirectory $captureDir `
        -PassThru
}
finally {
    Remove-Item Env:\TAPHLE_FRAME_CAPTURE_REQUEST -ErrorAction SilentlyContinue
    Remove-Item Env:\TAPHLE_FRAME_CAPTURE_OUTPUT -ErrorAction SilentlyContinue
}

# At the desired UI state, create the marker without replacing anything.
$marker = [IO.File]::Open(
    $requestPath,
    [IO.FileMode]::CreateNew,
    [IO.FileAccess]::Write,
    [IO.FileShare]::None
)
$marker.Dispose()

$deadline = [DateTime]::UtcNow.AddSeconds(10)
$captureComplete = $false
while (-not $captureComplete -and [DateTime]::UtcNow -lt $deadline) {
    if (Test-Path -LiteralPath $outputPath -PathType Leaf) {
        # The destination becomes visible before its payload is fully written,
        # so file existence alone is not completion.
        ffmpeg -v quiet -i $outputPath -f null -
        $captureComplete = $LASTEXITCODE -eq 0
    }
    Start-Sleep -Milliseconds 100
}
if (-not $captureComplete) {
    throw 'Timed out waiting for a complete, decodable frame capture'
}
```

The output path must not exist. tapHLE creates it with no-overwrite semantics,
logs the PPM dimensions on success, and makes only one attempt per marker. A
successful attempt removes the marker on a best-effort basis and re-arms the same
paths. To capture a later state in the same process, move the completed output to
a unique evidence filename, perform the next action, then create the same marker
again. A write failure, inaccessible path, invalid marker, or pre-existing output
is logged without crashing the emulator; that failure disarms the request and may
leave the marker in place, so restart tapHLE before retrying.

Wait for the output with a short deadline rather than an unbounded loop. Check
that the file begins with a `P6` header and that a decoder reports the expected
dimensions and complete pixel payload. The capture has OpenGL's pixel origin and
may need a vertical flip and an orientation-specific rotation for visual
inspection:

```powershell
ffmpeg -i $outputPath -vf 'vflip,transpose=2' (
    Join-Path $captureDir 'frame-oriented.png'
)
```

The synchronous readback can briefly stall one frame and temporarily holds both
RGBA and RGB copies in memory. Keep the PPM, converted image, marker, and raw
logs in the unique temporary run directory; never add them to Git or treat a
dirty-build capture as compatibility-database evidence.

### A frame capture is not necessarily the screen

tapHLE has two capture sites and they answer different questions.

`capture_renderbuffer` logs **"Captured submitted EAGL renderbuffer"**. It runs
inside `-[EAGLContext presentRenderbuffer:]` *before* the fast/slow branch, so it
records what the app drew into its own buffer — before host rotation, before
composition, before the virtual cursor.

`capture_composited_frame` logs **"Captured presented Core Animation frame"**.
That one is the screen, after host rotation and UIKit layer composition, and is
the better check for a menu or other UIKit-only screen that has stopped
submitting EAGL frames.

The trap: when an app has no visible window, `recomposite_if_necessary` returns
early and `find_fullscreen_eagl_layer` returns nil, so nothing is presented at
all — and the *renderbuffer* capture still produces a perfect-looking image. The
Jim & Frank Mysteries HD was rated two stars on that basis for a whole session.
Its window came from its main nib and was being deallocated, so the screen held a
frozen splash while every capture showed a full main menu.

Two consequences:

1. **Check which line the log emitted.** `grep Captured` costs nothing and tells
   you which of the two you are holding.
2. **For anything surprising, take an OS-level screenshot.** `PrintWindow` with
   `PW_RENDERFULLCONTENT` on the tapHLE window is outside tapHLE's GL code
   entirely, so it cannot be fooled by any of this. Sampling the pixel histogram
   is enough — a frozen splash, a grey fill and a live app are obvious apart by
   distinct-colour count alone.

A corollary: a rotated capture proves nothing about orientation, because the
renderbuffer capture is taken before host rotation. Neither does an unchanged
capture after passing `--landscape-native`, which only affects `present_frame`.

**Read frames; do not bucket them by mean brightness.** That misread live
gameplay as a stuck screen. And the maintainer's eyes outrank any sampling.

## Shutdown is an observable boundary

Test process shutdown separately after reaching a meaningful milestone. Post
`WM_CLOSE` only to the verified window owned by the spawned tapHLE PID, allow a
short fixed grace period, and record both the process exit code and whether the
harness had to kill it. A lifecycle callback or the absence of a Rust panic is
not sufficient evidence of a clean exit.

On Windows, take an Application event-log baseline before posting the close and
then query new event ID 1000 entries for `tapHLE.exe`. Native structured
exception handling or coroutine forced unwinding can produce an access violation
after guest shutdown logs appear normal. A clean result requires a natural zero
exit, no forced termination, and no new matching crash event. Use a fresh run
directory when confirming a fix so persisted app state cannot mask the path.

PowerShell scripts that launch the emulator through
`System.Diagnostics.Process` may leave `$LASTEXITCODE` unset even after a
successful run. Validate with `$?` and the script's explicit `EXIT_CODE` and
`FORCED_TERMINATION` fields instead of turning an unset value into a false
failure.

When shutdown reaches a missing thread-termination API, first establish the guest
thread ID and whether the call comes from the pthread's top-level start routine
or from a nested host-to-guest callback. These paths cannot be assumed to unwind
the same host coroutine. Prefer a normal return from the coroutine for the proven
path; forced unwinding suspended Windows coroutines can fault in the host
runtime.

Account for dynamic-link stub shape. A four-byte lazy symbol stub branches to the
guest LR after its host implementation returns, while larger stubs may continue
at the PC set by that implementation. A non-returning HLE function must redirect
all control-flow state required by each supported stub shape. State the boundary
explicitly: supporting top-level secondary-thread exit does not imply support for
main-thread exit, nested callbacks, pthread cleanup handlers, or thread-specific
data destructors.

## Verifying streaming audio in layers

Separate four questions: did the decoder produce PCM, did buffers recycle, did
the host mixer emit signal, and did a person hear sane audio? One observation
does not automatically answer all four.

This cuts both ways, and the negative direction is the easier mistake. **A log
full of failed loads is not evidence that the app is silent.** An app may reach
audio by more than one route — a failing `AudioFileOpenURL()` loader and a working
OpenAL streaming path can coexist in the same title — so a repeated warning bounds
which loader failed and nothing more. Baby Monkey was recorded as having "no
audio" on exactly that reasoning while it was in fact playing music throughout;
the wave capture below settled it in one run, and the maintainer had simply heard
it.

**Never state what a user will hear, see, or be unable to do without an
observation at that layer.** If measuring is not practical, write down the
API-level fact you actually have and say the user-facing consequence is unknown.

The bundled OpenAL Soft Wave File Writer can capture tapHLE's process-local mixed
output without a microphone or system loopback. In a unique run directory, create
an OpenAL config with a new output path and launch tapHLE with `ALSOFT_CONF`
pointing to it and `ALSOFT_DRIVERS=wave`. Use stereo, signed 16-bit output at the
app's sample rate when possible. Assert that the target WAV does not exist first:
the backend overwrites it without prompting. This backend replaces speaker output
for the run.

Use `ffprobe` to validate the resulting codec, rate, channel count, duration, and
file size. Use `ffmpeg` `silencedetect`, `volumedetect`, or `astats` over bounded
time windows rather than inspecting raw samples. For a streaming queue, calculate
how much audio its initial buffers contain. Meaningful signal beyond that
duration supports continuity, but pair it with a module-scoped queue trace when
the conclusion matters. A strong lifecycle trace shows each guest buffer
repeating this chain:

```text
processed by OpenAL -> guest callback -> re-enqueue -> decode
```

Count exact buffer references, decode/recycle events, decoder warnings, and
source-underrun restarts. Do not dump encoded packets or PCM.

tapHLE may open separate OpenAL contexts for guest OpenAL and internal Audio
Toolbox. OpenAL Soft's wave backend uses one global output filename, so two
devices can truncate or overwrite regions of the same file. Do not interpret that
file's timeline by itself when multiple devices appear in the OpenAL log; use the
queue lifecycle trace to attribute continuity. A wave capture proves host-mixer
signal, not default-device routing or subjective audio quality. Keep the
compatibility feature `partial` until listening or another trustworthy end-to-end
observation confirms sane audible output.

Close only the spawned process and allow a bounded grace period so the backend
can finalize RIFF lengths. If shutdown panics or needs forced termination, verify
the header and duration before using the capture and record that limitation.
Never silently repair or classify an ambiguous file as evidence.

## A suite that aborts has not run the tests after the abort

`test_NSRunLoop_runMode` panicked under the headless harness, and the integration
suite stopped there — at test 125 of 142. The seventeen tests behind it had never
run, including two added afterwards, and one of them was failing. For weeks the
honest summary was "the integration test fails", which sounded like one known
problem and was in fact one known problem hiding an unknown number.

So when a suite aborts rather than reporting failures, treat everything after the
abort as **unverified, not passing**. Fix the abort first, then read the result;
and be suspicious of a newly added test that has only ever been seen to compile.

## Conserve time and disk

- Check free space before a large build or extraction when the drive has been
  under pressure.
- Reuse Cargo's incremental/target cache; do not run `cargo clean` as routine
  troubleshooting.
- Build once for a batch of trace questions instead of rebuilding after every log
  line.
- Filter a large log for the exact symbol, status code, or subsystem plus a few
  context lines; do not repeatedly reread or paste the full file.
- Prefer junctions/hard links over copied runtime support trees.
- Keep the small sanitized conclusion, not gigabytes of intermediate data.

**Remove only the exact temporary directories you created.** Never use a broad
wildcard or an unresolved path. The practice that makes this automatic: the
harness that creates a directory captures its path in a variable and is the only
thing that deletes it, by that variable. Enumerating the temporary directory by a
`taphle-*` pattern is the mistake this rule exists to prevent — that directory is
shared by every session and every app, so a matching name is not evidence that
the directory is yours, and a force delete there does not go to the Recycle Bin.
If you are not certain a directory is yours, leave it: stale temporary data costs
nothing, and it may be another investigation's only remaining evidence.

### Stale build metadata is not an app result

Rust and Cargo may retain absolute paths to an old worktree in `target` build
metadata. If a diagnostic build unexpectedly names a deleted sibling checkout, do
not recreate that sibling. Confirm the current repository and target path, then
invalidate only the affected cached build scope.

**Never treat a failure caused by stale build metadata or a missing optional
debug dependency as an app compatibility result.**

Prefer fixing such a reference at its source over working around it: a build
script that locates files through `env!("CARGO_MANIFEST_DIR")` bakes the
compiling worktree's absolute path into the build-script binary, which cargo can
reuse from a since-deleted worktree. Reading `CARGO_MANIFEST_DIR` at run time
(`std::env::var`) is stale-proof because cargo sets it fresh per invocation.

The native wrapper crates (`dynarmic`, `openal`) build their C/C++ via CMake and
need `cmake` on `PATH`; a from-scratch debug build of those crates will fail
without it.

To build while the maintainer's tree is dirty, use a separate worktree with its
own submodule init and its own `CARGO_TARGET_DIR`. Never stash their work.

After a commit, a binary can still report the old revision; touch
`src/version/build.rs` to force the stamp to regenerate.
