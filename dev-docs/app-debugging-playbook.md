# Efficient Windows app troubleshooting

This is the operational playbook for advancing a selected game without
repeating settled work. Read it with `AGENTS.md`, `agent-workflow.md`, and the
app's sanitized note under `dev-docs/app-notes/` when one exists.

## Resume from evidence, not from launch

Before building or running anything:

1. Check the current branch, worktree, recent commits, and remote tracking
   state.
2. Read the app's compatibility record and continuation note.
3. Identify the highest milestone already reproduced on a committed build.
4. Write down the one next observable boundary. Examples are “menu tap reaches
   level select,” “the queue starts,” or “the first frame presents.”
5. Choose the cheapest observation that distinguishes the live hypotheses.

Do not replay the whole investigation merely to become familiar with it. Start
from the last trustworthy milestone and test only the next frontier unless a
code change could have regressed an earlier boundary.

Keep three small lists while working:

- **Proven:** directly supported by a hash-verified run, static inspection, or
  a deterministic test.
- **Rejected:** hypotheses contradicted by evidence, including why.
- **Next discriminator:** one bounded trace, test, or input that decides what
  to implement next.

If work crosses agents or sessions, update the app note before stopping. The
note is a continuation aid, not a compatibility claim.

## Keep app debugging visible

Run compatibility targets in a normal, visible tapHLE window by default. The
maintainer should be able to watch startup, rendering, orientation changes,
automated input, crashes, and newly reached screens while an agent works. Do
not hide an app run with `--headless`, a background-only desktop, or an
off-screen window unless the named experiment is genuinely independent of
UIKit, input, and graphics.

Agents may capture frames and click tested client coordinates to make repeated
runs objective and efficient. Keep the emulator window visible while doing so,
show one boundary at a time, and record the coordinate/timing recipe. Automated
screenshots and clicks support what a person can observe; they do not replace a
human playtest for menu, input, gameplay, orientation, or audio claims.

### A tap that does nothing is usually the harness, not the app

On a display with scaling — this one runs at 175% — tapHLE's window is
DPI-unaware, so Windows virtualises its coordinates. A harness that is itself
DPI-aware and converts client coordinates with `ClientToScreen` therefore puts
the cursor well away from the button it aimed at, and every control in every app
looks unresponsive.

That cost four apps a wrong conclusion in one session on 2026-08-16, and the
wrong conclusion was convincing: the touch really was delivered, the app's
`hitTest:` and `pointInside:` really did run, and they honestly answered "not in
this button". One of those apps went from "its buttons do not respond" to three
stars on the strength of the coordinate change alone.

So: **measure the target on a screen capture and click at that offset from the
window's origin.** Then diff the frames either side of the tap; a tap that
changed nothing is a fact about the pixels, not yet a fact about the app. If
several apps in a row appear to ignore taps, suspect the harness first.

Note that a clickmap records *client* coordinates by design, so its numbers have
to be converted before a harness of this kind can use them.

### Screen capture, not PrintWindow, for anything drawn with OpenGL

`PrintWindow` copies a window's GDI surface. An accelerated OpenGL window has
nothing there, so it can come back black while the game is plainly visible.
Copy from the screen instead when the result matters, and treat a black
`PrintWindow` capture of a GL app as no evidence at all.

### Antigravity CLI interactive-desktop harness

This harness is for **Google Antigravity CLI (AGY) only**. Codex and other
agent surfaces must not use `agy-visible-taphle.ps1`; they should use their own
visible-window launch, inspection, input, and shutdown facilities instead.

AGY's ordinary Windows commands run on a background desktop. Direct execution,
`Start-Process`, and `cmd /c start` can initialize tapHLE's SDL, OpenGL, and
audio paths while leaving its window invisible to the logged-in user. AGY must
use the repository harness for all GUI-sensitive operations.

Run exactly one step at a time and stop on any nonzero exit:

```powershell
# Build once.
cargo build --release

# Launch with -AppPath set to the exact verified IPA path.
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Launch

# Require a live PID and nonzero top-level window handle.
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Status

# Capture and inspect the starting screen.
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Capture

# Focus and click one client coordinate.
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Focus
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Click -X 384 -Y 512

# Capture and inspect the resulting screen before recording the click-map step.
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Capture

# Close only the broker-launched process.
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Close
```

For a direct launch, use a literal path and never guess an Archive filename:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File `
    .\dev-scripts\agy-visible-taphle.ps1 -Action Launch `
    -AppPath '.\tapHLE_apps\<exact verified filename>.ipa'
```

The harness creates per-user Scheduled Tasks with `Interactive` logon type.
The launch task keeps tapHLE on the logged-in desktop and configures tapHLE's
internal frame capture. The short-lived input task focuses, clicks, or posts
`WM_CLOSE` on that same desktop. State and logs live outside the checkout under
`%LOCALAPPDATA%\tapHLE\agy-visible`.

A click-map step is proven only when `Status` returns a nonzero window handle,
the pre-click frame visibly identifies the expected starting screen, `Click`
succeeds, and the post-click frame visibly identifies the resulting screen.
Audio, a zero command exit, or a running background task is not visual proof.
Use `-Action Uninstall` only to intentionally remove the scheduled tasks.

## Freeze the artifact identity first

Read the app's identity from `tapHLE --info` — bundle identifier, bundle
version, short version, minimum OS version — and record it in the app note.
**A filename is not identity**, and neither is a download page title or your
recollection of which app you launched. Do this once per artifact, and re-read it
before composing any report.

For an Archive-backed target, use the item URL supplied by the maintainer; never
search for a substitute. Record where the file came from so someone else can
obtain it, and record a locally computed SHA-256 so a later run can confirm it is
testing the same bytes:

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath '<exact local IPA>'
```

That comparison — this run against an earlier run — is the one worth making.
Matching a fresh download against the same host's published MD5/SHA-1 is not
required and no longer gates anything; see "Why the download hash gate is gone"
in `compatibility/README.md`. A missing or mismatched published hash is not a
reason to refuse to test a file.

Do not duplicate IPAs between run directories. Keep them outside Git and pass
their absolute path to tapHLE.

## Check the binary is decrypted before blaming the emulator

An App Store binary ships FairPlay-encrypted. A dump obtained without
decryption still has `LC_ENCRYPTION_INFO` with `cryptid = 1`, and its `__TEXT`
is ciphertext. tapHLE will load such a slice happily and then execute garbage.

The signature is distinctive and easy to misread as an emulator bug: the last
line logged is `Loading <arch> slice for "<app>"`, and then the process dies or
hangs with **no Rust panic**, no register dump, and nothing on stderr even with
`RUST_BACKTRACE=1`. There is no missing selector to chase, because no guest code
has meaningfully run.

Check `cryptid` before investigating any load-time failure of that shape. From
the IPA, without extracting anything to disk:

```python
import zipfile, struct
d = zipfile.ZipFile(ipa).read(exe_member)
# walk the fat header if magic is 0xcafebabe, then per slice walk the load
# commands and look for LC_ENCRYPTION_INFO (0x21) / _64 (0x2C); the cryptid is
# the u32 at offset 16 in that command.
```

`cryptid = 1` means the artifact cannot be tested, whatever the emulator does.
That is a fact about the file, not a compatibility result: record it as the
reason the app is untestable and move on, rather than filing it as an emulator
limitation. Two apps in one session (JungleZuma, Max Adventure Free) were
misdiagnosed as a shared Mach-O loader bug before this was checked.

## Follow an evidence ladder

Use the lowest-cost layer that can answer the current question:

1. Existing log or compatibility observation.
2. Source search for the exact symbol, status code, selector, or format.
3. Static inspection of the authorized app to recover a branch condition,
   call target, UI hit box, or faulting symbol.
4. A focused unit or TestApp regression.
5. A bounded diagnostic build and one controlled Windows run.
6. A clean committed release build and exact-artifact milestone run.

Static inspection and runtime tracing complement each other. Static work is
often faster for questions such as “what input advances this screen?” Runtime
work is required for questions such as “does that input reach the next level?”
Do not guess at hidden buttons, timers, formats, or ABI behavior through dozens
of blind runs when one branch condition or structure dump can answer it.

Keep static work equally bounded. Extract only the embedded `Info.plist` and
executable needed to answer the named question into a unique `%TEMP%`
directory; do not unpack an entire library of apps. Record the summarized
condition or symbol, then remove the exact temporary directory when it is no
longer needed.

For an archived UIKit screen, diagnose drawing and hit testing separately.
`UISubviews` is stored back-to-front, so a decorative full-screen view near the
end can intercept every control even when the buttons render correctly. Check
the exact archived interaction flag and any content wrapper objects (for
example, image-only button content) before changing event routing or drawing.
When a missing keyed value and an explicit false value have different
semantics, use `containsValueForKey:` before decoding; otherwise a decoder can
silently replace a subclass-specific default.

Treat layout as a separate runtime boundary too. A view created after launch
can be mounted, touchable, and still blank if its custom `layoutSubviews` never
runs. For an EAGL-backed view, distinguish these checkpoints in order:
display-link creation, layout-driven drawable allocation, a nonzero bound
renderbuffer, and `presentRenderbuffer:`. Do not paper over a missing layout
pass by calling every view's layout method every frame; that can make an app
destroy and recreate its framebuffer continuously. The current narrow
post-launch behavior lays out a newly mounted controller root once. Nested
runtime views and later geometry changes still need a future dirty-layout
implementation.

For a guest crash, resolve the program counter to its owning app image,
library, or HLE boundary before designing a fix. Inspect the register values
and object/allocation lifetime around that exact operation. A native library
fault, managed-code fault, and emulator panic have different recovery paths.

On Windows, first check the LLVM tools already installed with the pinned Rust
toolchain before installing another disassembler. After extracting only the
authorized app executable to a unique temporary directory, a narrow lookup is:

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
nearby methods and C++ functions. Keep only the small address context needed
to explain the fault; do not save or commit a full disassembly. Reconcile the
faulting operands with tapHLE's register dump. For an Objective-C++ object,
also check compiler-emitted `.cxx_construct` and `.cxx_destruct` methods before
treating a zeroed C++ container as valid initialized state.

When guest CPU execution fails without a debugger attached, current tapHLE
automatically prints all guest registers and the current stack trace before
panicking. Preserve that small diagnostic in normal builds: it can turn a null
memory error into a single disassembly lookup without a trace rebuild. If the
faulting instruction dereferences a zero register, inspect Mach-O non-lazy data
imports as well as lazy function imports. Compiler support such as
`___stack_chk_guard` is data, so a missing relocation may survive startup and
only crash later when generated code loads through the unresolved slot.

Rust and Cargo may retain absolute paths to an old worktree in `target` build
metadata. If a diagnostic build unexpectedly names a deleted sibling checkout,
do not recreate that sibling. Confirm the current repository and target path,
then invalidate only the affected cached build scope. Never treat a failure
caused by stale build metadata or a missing optional debug dependency as an app
compatibility result. Prefer fixing such a reference at its source over working
around it: a build script that locates files through `env!("CARGO_MANIFEST_DIR")`
bakes the compiling worktree's absolute path into the build-script binary, which
cargo can reuse from a since-deleted worktree; reading `CARGO_MANIFEST_DIR` at
run time (`std::env::var`) is stale-proof because cargo sets it fresh per
invocation. The native wrapper crates (`dynarmic`, `openal`) build their C/C++
via CMake and need `cmake` on `PATH`; a from-scratch debug build of those crates
will fail without it.

When the fault is an assertion at an HLE graphics boundary, recover the exact
guest API call and enum before relaxing it. Some old apps make invalid GL calls
that a real driver answers by recording `GL_INVALID_ENUM` and continuing. A
bounded emulator fix should preserve that guest-visible error behavior instead
of converting the app mistake into a host panic or ignoring all GL errors.

## Instrument one boundary at a time

Temporary diagnostics should answer one named question and produce little
output. Gate them by the exact format, API, queue, or bundle when practical.

Good diagnostic fields include:

- an API's input structure and status code;
- buffer byte size, count, pointer route, and a four-byte signature;
- the first few packet descriptors or offsets;
- one allocation size/address and its allocate/free/reuse events; or
- one UI phase, coordinate, and resulting level/screen transition.

Do not dump entire audio buffers, textures, guest memory, app binaries, or raw
logs. Never commit a diagnostic containing proprietary bytes or personal
paths. After the question is answered, remove trace-only code or convert the
small useful failure message into a normal diagnostic.

### Recognize compiler-created Objective-C blocks

Clang emits stack and global block literals directly in guest memory. They are
Objective-C-compatible objects, but they were not allocated through tapHLE's
normal object table. A high stack address receiving `copy`, combined with an
unhandled `__NSConcreteStackBlock` or `__NSConcreteGlobalBlock` relocation, is
evidence of a missing Blocks runtime rather than a corrupt ordinary object.

Trace only the final selector sequence and inspect the stable block prefix:
`isa`, flags, reserved word, invoke pointer and descriptor pointer. Implement
the documented Blocks ABI coherently: external block classes, stack-to-heap
copy by descriptor size, copy/dispose helpers, heap reference counts,
`_Block_object_assign`, `_Block_object_dispose`, and `__block` forwarding.
Returning the original stack pointer from `copy` may move one run forward but
leaves a use-after-return bug, so it is not a valid compatibility fix.

### Suspect the emulator's own iteration before the app's ownership

A message arriving at an object whose class differs from run to run is a
use-after-free. The tempting next step is to hunt for a retain the app is
missing, but check the other side first: does tapHLE deliver that message by
iterating a **copy** of a callback list it took before running guest code?

`NSNotificationCenter`, responder and delegate lists, timer and run-loop
queues all have to copy before dispatching, because delivery reenters the guest
and the guest may register or unregister during it. Copying is correct;
iterating the copy blindly is not. These lists deliberately hold unretained
references, so an entry that unregisters and is deallocated part-way through a
dispatch leaves the copy naming freed memory. Cocoa's guarantee that a removed
observer is not messaged is exactly what makes unregistering in a teardown
method safe, so a faithful implementation has to re-check each copied entry
against the live list before messaging it.

Two traps make this easy to misdiagnose:

- Instrumenting `retain`/`release` and attributing each call by whether a guest
  `LR` is available will label the emulator's own retain around a dispatch as
  guest code. A retain/release pair that brackets exactly one send, and nothing
  else, is more likely to be the host's dispatch guard than an app's.
- If the app routes its own event system through a Foundation class, the guest
  method names will not say so. Resolve the wrapper: a two-call method that
  fetches a singleton and then sends one selector to it is almost always a thin
  forwarder, and resolving its `__objc_selrefs` entries names the real API.

Read structures before reinterpreting data. For compressed audio, for example,
capture the `AudioStreamBasicDescription`, packet-description route/count, and
the first sync bytes before treating the buffer as PCM. For a stale-pointer
crash, prove allocation/free/reuse behavior before disabling an allocator rule
globally.

Use existing dependencies and subsystem abstractions before introducing a new
decoder, parser, or host library.

## Make Windows runs isolated and repeatable

Use a uniquely named directory under `%TEMP%` as tapHLE's working directory.
Link its `tapHLE_dylibs` and `tapHLE_fonts` to the checkout and copy the small
tracked default-options file rather than copying large support trees. Do not
add local options unless the experiment is specifically testing one.

Do not use `--headless` merely to hide an automated app run. UIKit and EAGL
paths can legitimately require `Environment.window`; headless mode may create
an earlier, unrelated unwrap or no-context failure. Use it only when the named
test is genuinely window-independent. A bounded normal-window run may redirect
logs and stop only its exact spawned PID.

For each meaningful run, record:

- exact tapHLE commit or that the build was dirty;
- verified IPA hash;
- tapHLE arguments and options source;
- client-area input coordinates and timing;
- last screen/event reached and whether the process stayed alive; and
- the narrow log lines supporting the conclusion.

Use real foreground mouse input when Windows message injection does not reach
SDL. Before every injected event, re-check that the foreground window belongs
to the exact spawned tapHLE process; abort input if focus or process identity
changed. Save and restore the previous cursor position and foreground window,
and stop only the exact process launched by the run. Visual inspection of a
screenshot can support a rendering observation, but the screenshot stays in
the ignored temporary directory.

If Windows foreground locking rejects `SetForegroundWindow`, temporarily
attach the harness thread's input queue to both the thread that currently owns
the foreground and the verified tapHLE window thread. Re-read both handles and
thread IDs on every bounded retry, detach in reverse order before sleeping, and
verify the exact target handle again after positioning the cursor and before
injecting input. Do not send a synthetic Alt key while an unrelated app owns
the foreground; it can alter that app's state. Abort safely if dual attachment
cannot establish ownership.

Allow the target event loop to consume the cursor move before sending the
button-down event (a short bounded pause such as 200 ms is sufficient in the
current harness). When touch routing is the boundary under test, confirm the
trace contains a `MouseMotion` at the intended client coordinate before the
matching `MouseButtonDown`. Sending the press immediately after
`SetCursorPos` can make SDL use the previous cursor coordinate and create a
false hit-testing diagnosis.

Also verify that `FocusGained` precedes the first intended client click.
Windows may consume the first click only to activate the emulator even when
`SetForegroundWindow` was requested. A reliable harness can click the spawned
window's title bar, wait for the focus event, and only then begin the recorded
client-coordinate recipe. Never use a focus click inside the app as if it were
part of the compatibility test.

Reuse a proven input recipe. Change one step at the frontier rather than
inventing a new route on every launch. A successful click path is evidence and
belongs in the app note as a **click map**: the ordered list of taps that
navigates the app from launch to its current frontier. Record one for every app
that needs more than a launch to reach its milestone, and keep it current as the
frontier advances. A good click map lets the next agent (or a re-verification
run) reproduce the milestone mechanically instead of re-discovering the route,
and it makes a "did this regress?" check objective.

**Write it as a clickmap.** `dev-docs/clickmaps/` holds these as JSON that
`dev-scripts/clickmap.ps1` can replay, so re-verification is a command rather
than a reading exercise, and a regression check names the step it stopped on.
Replay an existing map before working out a route by hand, including the map
for a different version of the same app. The format and its rules are in
`dev-docs/clickmaps/protocol.md`; everything below is what a good map records,
and the format has a field for each of it.

Each click-map step records: the screen it starts on, the client-area tap
coordinate, and the screen it leads to. Client coordinates are only meaningful
against a stated **window size and orientation**, because tapHLE maps a client
tap into app space through the viewport and rotation — a landscape-native
1024×768 window, a portrait 768×1024 window, and a scaled window all map the same
client point differently. So the header of a click map states the exact launch
options (e.g. `--landscape-native`), the resulting window size, and the menu-load
wait before the first tap. Prefer coordinates that hit the centre of a generous
target so small layout differences do not miss. When a tap is timing-sensitive,
note the wait after it.

### Capture the rendered frame when desktop capture is black

Windows desktop screenshot APIs may return a black OpenGL client area. tapHLE
can capture the next rendered frame after an explicit request marker appears.
EAGL apps capture the next valid renderbuffer submitted through
`presentRenderbuffer:`. UIKit-only screens capture the next frame presented by
the Core Animation compositor. This is a harness feature, not an app option:
set both paths in the child process environment before launch, then create the
request marker only after the target state is reached.

```powershell
$captureDir = Join-Path ([IO.Path]::GetTempPath()) (
    'taphle-frame-' + [Guid]::NewGuid().ToString('N')
)
[void](New-Item -ItemType Directory -Path $captureDir)
$tapHLEPath = '<absolute path to the exact committed tapHLE.exe>'
$ipaPath = '<absolute path to the exact hash-verified IPA>'
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
successful attempt removes the marker on a best-effort basis and re-arms the
same paths. To capture a later state in the same process, move the completed
output to a unique evidence filename, perform the next action, then create the
same marker again. A write failure, inaccessible path, invalid marker, or
pre-existing output is logged without crashing the emulator; that failure
disarms the request and may leave the marker in place, so restart tapHLE before
retrying.

Wait for the output with a short deadline rather than an unbounded loop. Check
that the file begins with a `P6` header and that a decoder reports the expected
dimensions and complete pixel payload. The capture has OpenGL's pixel origin
and may need a vertical flip and an orientation-specific rotation for visual
inspection. For example, Ricky's landscape-left diagnostic used:

```powershell
ffmpeg -i $outputPath -vf 'vflip,transpose=2' (
    Join-Path $captureDir 'frame-oriented.png'
)
```

Read the success log before interpreting the image because the rendering path
changes what was captured:

- `Captured submitted EAGL renderbuffer` is the guest renderbuffer before host
  rotation, layer composition, scaling, letterboxing, or virtual-cursor
  drawing. In a multi-layer app, it may not be the layer ultimately visible.
- `Captured presented Core Animation frame` is the presented viewport after
  host rotation and UIKit layer composition. This is the better check for a
  menu or other UIKit-only screen that stops submitting EAGL frames.

Both captures have OpenGL's bottom-left pixel origin, so start visual
conversion with `vflip`. Apply an additional rotation only to a pre-rotation
EAGL capture when its orientation requires one. The synchronous readback can
briefly stall one frame and temporarily holds both RGBA and RGB copies in
memory. Keep the PPM, converted image, marker, and raw logs in the unique
temporary run directory; never add them to Git or treat a dirty-build capture
as compatibility-database evidence.

## Treat shutdown as an observable boundary

Test process shutdown separately after reaching a meaningful milestone. Post
`WM_CLOSE` only to the verified window owned by the spawned tapHLE PID, allow a
short fixed grace period, and record both the process exit code and whether the
harness had to kill it. A lifecycle callback or the absence of a Rust panic is
not sufficient evidence of a clean exit.

On Windows, take an Application event-log baseline before posting the close and
then query new event ID 1000 entries for `tapHLE.exe`. Native structured
exception handling or coroutine forced unwinding can produce an access
violation after guest shutdown logs appear normal. A clean result requires a
natural zero exit, no forced termination, and no new matching crash event. Use
a fresh run directory when confirming a fix so persisted app state cannot mask
the path.

PowerShell scripts that launch the emulator through
`System.Diagnostics.Process` may leave `$LASTEXITCODE` unset even after a
successful run. Validate the script with `$?` and its explicit `EXIT_CODE` and
`FORCED_TERMINATION` fields instead of turning an unset value into a false
failure.

When shutdown reaches a missing thread-termination API, first establish the
guest thread ID and whether the call comes from the pthread's top-level start
routine or from a nested host-to-guest callback. These paths cannot be assumed
to unwind the same host coroutine. Prefer a normal return from the coroutine
for the proven path; forced unwinding suspended Windows coroutines can fault in
the host runtime.

Also account for dynamic-link stub shape. A four-byte lazy symbol stub branches
to the guest LR after its host implementation returns, while larger stubs may
continue at the PC set by that implementation. A non-returning HLE function
must redirect all control-flow state required by each supported stub shape.
State the boundary explicitly: supporting top-level secondary-thread exit does
not imply support for main-thread exit, nested callbacks, pthread cleanup
handlers, or thread-specific-data destructors.

## Verify streaming audio in layers

Separate four questions: did the decoder produce PCM, did buffers recycle, did
the host mixer emit signal, and did a person hear sane audio? One observation
does not automatically answer all four.

This cuts both ways, and the negative direction is the easier mistake. A log
full of failed loads is not evidence that the app is silent. An app may reach
audio by more than one route — a failing `AudioFileOpenURL()` loader and a
working OpenAL streaming path can coexist in the same title — so a repeated
warning bounds which loader failed and nothing more. Baby Monkey was recorded as
having "no audio" on exactly that reasoning while it was in fact playing music
throughout; the wave capture below settled it in one run, and the maintainer had
simply heard it. Never state what a user will hear, see, or be unable to do
without an observation at that layer. If measuring is not practical, write down
the API-level fact you actually have and say the user-facing consequence is
unknown.

The bundled OpenAL Soft Wave File Writer can capture tapHLE's process-local
mixed output without a microphone or system loopback. In a unique run
directory, create an OpenAL config with a new output path and launch tapHLE with
`ALSOFT_CONF` pointing to it and `ALSOFT_DRIVERS=wave`. Use stereo, signed
16-bit output at the app's sample rate when possible. Assert that the target
WAV does not exist first: the backend overwrites it without prompting. This
backend replaces speaker output for the run.

Use `ffprobe` to validate the resulting codec, rate, channel count, duration,
and file size. Use `ffmpeg` `silencedetect`, `volumedetect`, or `astats` over
bounded time windows rather than inspecting raw samples. For a streaming
queue, calculate how much audio its initial buffers contain. Meaningful signal
beyond that duration supports continuity, but pair it with a module-scoped
queue trace when the conclusion matters. A strong lifecycle trace shows each
guest buffer repeating this chain:

```text
processed by OpenAL -> guest callback -> re-enqueue -> decode
```

Count exact buffer references, decode/recycle events, decoder warnings, and
source-underrun restarts. Do not dump encoded packets or PCM.

tapHLE may open separate OpenAL contexts for guest OpenAL and internal Audio
Toolbox. OpenAL Soft's wave backend uses one global output filename, so two
devices can truncate or overwrite regions of the same file. Do not interpret
that file's timeline by itself when multiple devices appear in the OpenAL log;
use the queue lifecycle trace to attribute continuity. A wave capture proves
host-mixer signal, not default-device routing or subjective audio quality.
Keep the compatibility feature `partial` until listening or another trustworthy
end-to-end observation confirms sane audible output.

Close only the spawned process and allow a bounded grace period so the backend
can finalize RIFF lengths. If shutdown panics or needs forced termination,
verify the header and duration before using the capture and record that
limitation. Never silently repair or classify an ambiguous file as evidence.

## Conserve time and disk

- Check free space before a large build or extraction when the drive has been
  under pressure.
- Reuse Cargo's incremental/target cache; do not run `cargo clean` as routine
  troubleshooting.
- Build once for a batch of trace questions instead of rebuilding after every
  log line.
- Filter a large log for the exact symbol, status code, or subsystem plus a few
  context lines; do not repeatedly reread or paste the full file.
- Prefer junctions/hard links over copied runtime support trees.
- Remove only the exact temporary extraction/run directories created for the
  completed experiment. Never use a broad wildcard or an unresolved path. The
  practice that makes this automatic: the harness that creates a directory
  captures its path in a variable and is the only thing that deletes it, by that
  variable. Enumerating the temporary directory by a `taphle-*` pattern is the
  mistake this rule exists to prevent — that directory is shared by every
  session and every app, so a matching name is not evidence that the directory
  is yours, and a force delete there does not go to the Recycle Bin. If you are
  not certain a directory is yours, leave it: stale temporary data costs
  nothing, and it may be another investigation's only remaining evidence.
- Keep the small sanitized conclusion, not gigabytes of intermediate data.

## Choose and bound the fix

The fix may be general, intentionally partial, or game-specific. Prefer the
smallest complete behavior supported by evidence.

Treat a later crash as a new observation, not automatically as progress. A
useful step implements or corrects a coherent guest-visible capability and
proves that the target crosses that boundary. Suppressing several imports with
unrelated success stubs is not a checkpoint; preserve such experiments only as
leads and review each API contract separately.

- A general implementation should validate its inputs and fail safely for
  unsupported variants.
- A partial implementation should state the exact supported shape and leave a
  visible fallback/TODO for other routes.
- A game workaround should be gated as narrowly as possible and explain the
  observed behavior it preserves.

Do not broaden scope to adjacent formats, platforms, or APIs just because they
are nearby. An implementation for the route the target actually uses is a
valid checkpoint. Add a synthetic regression that contains no proprietary
fixture whenever deterministic logic can be isolated.

## Commit, then make compatibility claims

A dirty-worktree run is a useful experiment but never database evidence.

1. Commit the focused implementation on `compat/<app-slug>`.
2. Build and run the relevant tests from that exact commit.
3. Re-verify the IPA hash and replay the milestone on the claimed host.
4. If the milestone is reproducible, append a compatibility report referencing
   the tested implementation commit.
5. Submit the verified result to the live compatibility database when
   publication is authorized.
6. When publishing is authorized, push unfinished checkpoints to the
   compatibility branch so another agent can resume them. Merge a stable,
   documented milestone to `trunk` even when known limitations remain.

Keep implementation, agent-policy documentation, and compatibility reports in
separable commits. This makes incomplete app experiments easy to continue or
revert without losing durable process improvements.

### Include a screenshot when you have one

A screenshot of the milestone is welcome in a report and makes the rating much
easier for a human to confirm or overturn — a picture of the running game
settles "in game" faster than any amount of prose about it. Prefer the OS-level
screenshot of tapHLE's window over tapHLE's own frame capture, for the reason
in "A frame capture is not necessarily the screen".

It is **not required**. Some milestones are not visual at all, desktop capture
sometimes returns a black client area, and a report is perfectly valid without
one. Never hold back or delay a verified result because a screenshot could not
be obtained, and never describe a screen you did not actually look at in order
to have something to say — an accurate report with no image beats an
illustrated guess. Say what you observed and how you observed it.

## Continuation-note template

Create `dev-docs/app-notes/<app-slug>.md` on the app branch when work will span
more than one focused turn. Keep it sanitized and concise:

```markdown
# <App> compatibility work note

- Branch and last pushed commit:
- Canonical artifact URL, filename, SHA-256, bundle ID/version:
- Highest clean committed milestone:
- Click map (launch options, window size/orientation, menu-load wait, then the
  ordered `screen -> client (x,y) -> resulting screen` taps):
- Proven facts:
- Rejected hypotheses:
- Current uncommitted diagnostics or code:
- Checks already run:
- Known risks/regressions to watch:
- Next discriminator or implementation step:
```

The click map is the reproducible route from launch to the frontier; see "Reuse
a proven input recipe" above for what each step records and why the window
size/orientation header is required.

Do not put app code, decompiled listings, assets, screenshots, raw logs, keys,
or personal paths in the note. Exact public provenance, hashes, symbol names,
addresses, API shapes, and summarized behavior are sufficient for continuity.

## A liveness check is not a regression check

Two three-star apps were broken during a single session and neither was caught
by the routine sweep. Both misses were the same mistake in different clothes.

**SPY mouse HD** stayed up for roughly forty seconds and then aborted. A sweep
that launches an app, waits twenty seconds and confirms the process is alive saw
a healthy splash screen and reported success. The app never reached a level
again.

**JellyCar 2** was not in the sweep at all. It had been rated three stars weeks
earlier, so it was assumed to still work — and it had been dead for a dozen
commits before anyone launched it.

So:

1. **Check every app that carries a rating**, not a convenient subset. An app
   absent from the sweep is an app whose rating is a claim about the past.
2. **Drive each one to the milestone it is rated for.** Three stars means a
   gameplay loop, so the check has to reach gameplay. Confirming the process is
   alive confirms only that it has not crashed *yet*.
3. **Wait long enough.** Give an app at least as long as its recorded startup
   time plus a margin. Failures that arrive at forty seconds are invisible to a
   twenty-second check.
4. **Compare frames by hash, not by eye or by file size.** Two captures of a
   static screen are byte-identical; two captures of a running game are not.
   That single check distinguishes "playing" from "frozen on a plausible
   screenshot", and it is the check that has caught the most.

`dev-scripts/regression-sweep.ps1` does 1, 3 and 4. It sweeps whatever is in
the app collection directory, so there is no list for an app to fall out of,
and it takes per-app launch options from `tapHLE_default_options.txt` by bundle
identifier rather than carrying its own copy of them. It exits non-zero if any
app failed to start or died.

Run it before merging anything that touches a shared path, and after. Writing
the check by hand instead is how it shrank to eight apps and a twenty-second
wait the last time.

It does **not** do 2. Nothing in it leaves the title screen, so it reports
`STATIC` for an app that is merely waiting for input and for one that is
wedged, and cannot tell them apart. Reaching a rated milestone still means
following that app's click map by hand.

Treat `STATIC` as a prompt to look, not as a verdict. Some title screens change
very slowly - Warlords HD's takes about fifteen seconds - so the sweep samples
several frames before calling anything still, and even then Flight Control HD
has read `MOVING` on one run and `STATIC` on the next. That is why `STATIC`
does not fail the run. A genuine freeze is confirmed by driving the app, not by
this number.

**Run it on a machine nobody is using.** Every app appears on the real desktop
and accepts real input, for around half an hour. JellyCar 2 was recorded as
`MOVING` on one run because the maintainer picked it up and played it, when its
title screen does not move on its own. The sweep now checks how long the
machine has been idle and marks any such result `MOVING?`, but it can only
report the doubt, not remove it: a run taken while the desktop was in use
cannot establish that anything still animates.

### Bisect rather than guess when a regression appears

An app that used to work and now does not has a first bad commit, and finding it
is cheap next to reasoning about which change "looks risky". JellyCar 2 was
pinned in three builds — working at its rated commit, working at the parent of
the suspect merge, dead at the merge — which named the cause with no argument
about it.

The rated commit is recorded in every app note precisely so this is possible.
Use it.

### The fix for a regression is rarely "revert" or "keep"

The layout-on-mount change that broke JellyCar 2 was also what got Tap Tap
Revenge 2 into gameplay, so both reverting and keeping it cost a three-star
result. Two narrower attempts failed — dropping the recursion into subviews, and
restricting it to `CAEAGLLayer`-backed views — and their failure is what showed
the problem was *when* the layout ran rather than *which views* it touched.

The answer was a condition neither app suggested on its own: lay out on mount
only once launching has finished. When two apps disagree, look for the
distinction that explains both, and treat a narrowing that fails as evidence
about the shape of the bug rather than a dead end.

## A frame capture is not necessarily the screen

tapHLE has two capture sites and they answer different questions.

`capture_renderbuffer` logs **"Captured submitted EAGL renderbuffer"**. It runs
inside `-[EAGLContext presentRenderbuffer:]` *before* the fast/slow branch, so it
records what the app drew into its own buffer — before host rotation, before
composition, before the virtual cursor. Its doc comment says exactly this.

`capture_composited_frame` logs **"Captured presented Core Animation frame"**.
That one is the screen.

The trap: when an app has no visible window, `recomposite_if_necessary` returns
early and `find_fullscreen_eagl_layer` returns nil, so nothing is presented at
all — and the *renderbuffer* capture still produces a perfect-looking image. The
Jim & Frank Mysteries HD was rated two stars on that basis for a whole session.
Its window comes from its main nib and was being deallocated, so the screen held
a frozen splash while every capture showed a full main menu.

Two consequences worth remembering:

1. **Check which line the log emitted.** `grep Captured` costs nothing and tells
   you which of the two you are holding.
2. **For anything surprising, take an OS-level screenshot.** `PrintWindow` with
   `PW_RENDERFULLCONTENT` on the tapHLE window is outside tapHLE's GL code
   entirely, so it cannot be fooled by any of this. Sampling the pixel histogram
   is enough — a frozen splash, a grey fill and a live game are obvious apart by
   distinct-colour count alone.

A corollary: a rotated capture proves nothing about orientation, because the
renderbuffer capture is taken before host rotation. Neither does an unchanged
capture after passing `--landscape-native`, which only affects `present_frame`.

## An opaque guest MemoryError is usually a name you already have

`Error during CPU execution: MemoryError` is what tapHLE reports for *any* bad
guest access, so it names nothing by itself. Two cheap steps turn it into a
symbol, and the second is nearly free.

### 1. Disassemble the faulting PC

`dev-scripts/disasm-guest-fault.py <app.ipa> <pc>` takes the PC straight from
tapHLE's register dump and prints the instructions around it, resolving the
guest address through the Mach-O segments and handling fat binaries and Thumb.

JellyCar 1 faulted at `0x30190`:

```text
0x0003018e  ldr   r3, [r3]        <- r3 = *(a global)
0x00030190  vldr  d7, [r3]        <== FAULT
```

with `R3 = 0` in the dump. So a global pointer was null and the code loaded a
double through it.

### 2. Cross-reference the address against the non-lazy symbol warnings

This is the step to try **first**, because it needs no tools at all. tapHLE
already logs, for every symbol it could not bind:

```text
Warning: unhandled non-lazy symbol "_kCFAbsoluteTimeIntervalSince1970" at 0x433e8
```

That address is the same one the faulting instruction loads from. The warning had
been in the log the whole time, filtered out as noise because these warnings are
common and usually harmless.

**They are only harmless while the guest does not dereference them.** An unbound
`__nl_symbol_ptr` slot is a null pointer sitting in `__DATA` waiting for someone
to read it, and the guest reads it without checking. So treat each warning as a
latent null dereference, and when a `MemoryError` appears, grep the log for the
address the fault touched before doing anything else.

Exporting that one constant took JellyCar 1 from crashing during startup to its
full main menu and into level loading.
