# Ricky compatibility work note

Last updated: 2026-07-18.

## Identity and branch

- Work branch: `compat/ricky`.
- Initial troubleshooting-playbook checkpoint: `4f6337fc`.
- Canonical item: `https://archive.org/details/ios-ipa-com.nabilchatbi.ricky`.
- Exact local/canonical filename used for runs:
  `Ricky (v2.1) [Cracked].ipa`.
- SHA-256:
  `74380d6f009a43d430fe3bceeccbb1c9d2b20f40a02088688b500b71eb937a5f`.
- Bundle: `com.nabilchatbi.Ricky`, version `2.1`, minimum OS `3.0`.
- Never use the older local `Ricky (v2.1) [Decrypted].ipa`; it does not match
  the canonical artifact hash.

## Highest gameplay milestone

Commit `d9bc6a5d02322f1902e14ba4a451e8b3a4ade2ee` launches the exact artifact,
navigates Story Mode, enters the first playable level, is sent a two-second hold
on the on-screen right movement control, and remains alive for
approximately 18 seconds after level entry on Windows. The same route previously
reached `AudioQueuePrime` and panicked; this clean run primed the packetized MP3
queue without error `-66681`, MP3 decode warnings, or the non-null output-pointer
panic. Compatibility report `2026-07-17-windows-story-in-game` supersedes the
earlier menu-only report.

The earlier clean `46dce243` runtime visually established that the level
rendered, the character moved, and the scene scrolled. A System.Drawing
`Graphics.CopyFromScreen` attempt on the `d9bc6a5d` OpenGL window produced a
black bitmap and was excluded from evidence; agents should not repeat that
capture method as proof of rendering. It is not evidence that the rendered
window was black. Audible audio output has not been confirmed, so audio remains
`partial` rather than `working`.

The runtime host used for this observation was Windows 11 Pro 10.0.22631 on an
Intel i7-13700H with Intel Iris Xe driver 32.0.101.6881.

The full Archive.org verification protocol passed immediately before the
`d9bc6a5d` run, including remote metadata hashes, local SHA-256, embedded bundle
identity, and `tapHLE --info` cross-checks.

## Repeatable input recipe

The route is recorded as `dev-docs/clickmaps/ricky.json` and was replayed end to end
on `e4e0d528` on 2026-08-17. Replay the map rather than retyping the steps
below; the prose is kept for the reasoning behind the coordinates, and the map
is what should be kept correct.


The tapHLE client area is 480 by 320. The coordinates below are client
coordinates and are valid only for that client size. Use foreground mouse
input, verify the window still belongs to the spawned tapHLE process before
every event, and restore the previous cursor/window afterward:

1. Acquire and verify foreground ownership, then tap Story Mode at `(75, 295)`
   once. Do not use an unconditional double tap; an earlier run needed a second
   click only because its first click activated the window.
2. New Game at `(397, 295)`.
3. PLAY at `(236, 203)`.
4. Advance the story at `(240, 295)` exactly twice.
5. Hold the right movement control near `(137, 289)` to verify gameplay input.

Static inspection established that the story screen advances only on a touch
begin in its bottom 50-pixel strip. It has three textures and loads level 2
after two qualifying taps; blind center-screen tapping and waiting are rejected
strategies.

## First-level controls and capture evidence

A state-aware internal EAGL capture on a dirty diagnostic build established the
first-level control targets at the 480 by 320 landscape-left client size:

- left movement: `(40, 285)`;
- right movement: `(130, 285)`;
- B/action: `(355, 285)`;
- A/jump: `(444, 285)`; and
- pause: `(16, 16)`.

Holding A visibly highlighted the A target and moved Ricky roughly 90 pixels
upward, directly establishing the jump action. Holding B visibly highlighted
the B target, which establishes touch acceptance but not the resulting action;
no projectile was visible in the bounded before/held/after frames. The earlier
clean run already established rightward movement. Leftward movement, B's full
behavior, and pause/resume still need direct runtime checks.

Commit `cdc8aab0` converts the diagnostic into a general, safe one-shot frame
capture protocol documented in `dev-docs/app-debugging-playbook.md`. The dirty
captures remain provisional debugging evidence and are not compatibility
database evidence. A clean committed run should use the new protocol when a
visual claim is ready to record. Desktop `Graphics.CopyFromScreen` remains a
rejected capture method for this OpenGL window.

## Proven fixes and risks

- `9b31d508`: `dlsym(RTLD_DEFAULT)` resolves symbols from loaded app binaries
  and returns null for a missing symbol.
- `46dce243`: Ricky zeroes new allocations and quarantines freed allocator
  chunks of exactly `0x40` bytes. This avoids two Unity delayed-call red-black
  tree crashes. The quarantine retains those chunks for the process lifetime,
  so long-session memory growth remains a risk to test.
- `fad70bc1`: AudioQueue owns Ricky's transient direct packet descriptions and
  decodes packetized MP3 through a persistent Symphonia decoder so the Layer III
  bit reservoir survives across queue buffers.
- `d9bc6a5d`: `AudioQueuePrime` accepts Ricky's non-null prepared-frame output
  pointer, implements zero/requested-frame preparation semantics, and reports
  the number of frames prepared.
- `e2d51c6c`: `_pthread_exit` terminates Ricky's top-level secondary Mono
  pthread through the existing guest thread-exit trampoline. A UIKit close
  request is persisted for the exact callback-carrying thread so that thread
  completion finishes host shutdown without forced coroutine unwinding.
- A Mono signal/exception recovery implementation would not fix the observed
  native bundled-libstdc++ tree fault; that path was investigated and rejected.

## Resolved audio failure path

A bounded diagnostic trace reached the first playable level with the canonical
hash and proved:

- the AudioQueue format is genuine MPEG-1 Layer III, 44.1 kHz stereo, 1,152
  frames per packet (`.mp3`), not float PCM or mislabeled PCM;
- Ricky allocates three regular 16,384-byte queue buffers;
- each enqueue supplies 39 **direct** packet descriptions through a transient,
  reused pointer; embedded descriptions are null/zero;
- encoded frames are contiguous 417/418-byte packets beginning with MP3 sync
  bytes; and
- the old unsupported-format path produced AudioQueue error `-66681`.

Rejected approaches: relabeling the data as PCM, decoding an entire queue buffer
as one MP3 packet, or creating a new decoder per buffer. Packet descriptions
must be copied at enqueue time, and one stateful decoder must survive across
buffers for the Layer III bit reservoir.

Commits `fad70bc1` and `d9bc6a5d` implement that design. Focused AudioQueue
tests pass, the full release library suite passed after MP3 integration, and an
exact-commit runtime crossed the former failure point without the old error or
panic. Embedded packet descriptions remain unsupported, but Ricky supplies
direct descriptions and does not require that path.

## Audio continuity evidence

A clean `c31aabcf` run used OpenAL Soft's Wave File Writer at 44.1 kHz stereo
signed 16-bit output. The resulting valid 54.52-second file contained 25.5
continuous seconds of meaningful signal around -18 dB mean/-7 dB peak before
the dither-only tail. That duration is far beyond the approximately 3.06
seconds held by Ricky's initial three buffers.

Because Ricky also creates a separate guest OpenAL context and the wave backend
uses one global output filename, the capture timeline alone is not sufficient
attribution. A second bounded build enabled only the existing AudioQueue debug
module and proved the queue lifecycle directly:

- the three guest buffer references were each decoded exactly seven times;
- 19 processed buffers invoked Ricky's callback and were re-enqueued;
- 21 total MP3 buffer decodes completed; and
- there were no MP3 decode warnings, prime failures, or OpenAL source-underrun
  restarts.

This establishes non-silent host-mixer output and continuous callback-driven
MP3 streaming. It does not establish default Windows audio-device routing or
that a person heard artifact-free sound, so the compatibility feature remains
`partial`.

## Clean-exit result

The exact canonical IPA passed two fresh-sandbox, process-scoped `WM_CLOSE`
runs using the committed `e2d51c6c3f892275cdcfad8e43235d54d34eac11`
release build. The second run recorded all of the following after a ten-second
menu observation:

- the local IPA SHA-256 exactly matched the canonical value above;
- the launched binary identified itself as `e2d51c6c`;
- `applicationDidResignActive` and `applicationWillTerminate` ran;
- the process exited with code 0 inside the five-second grace period;
- the harness did not force-terminate the process;
- captured output contained no Rust panic or crash signature; and
- Windows recorded no new tapHLE Application Error event during the run.

The implementation is deliberately bounded to `_pthread_exit` reached from a
secondary pthread's top-level start routine, which is the path used by Ricky's
Mono wrapper. Main-thread exit, exit from nested host-to-guest callbacks,
pthread cleanup handlers, and thread-specific-data destructors remain
unsupported. The clean-exit run observed the menu rather than replaying the
full Story route, so it did not by itself replace the existing in-game
compatibility report.

A later fresh-sandbox run of the same exact build and artifact used the proven
foreground input recipe to replay Story Mode through the first playable level,
sent the two-second right-control hold, remained alive for another ten seconds,
and reached the harness's final stage. Its PID-verified close also exited with
code 0 without forced termination, panic/crash output, or a new Windows
Application Error event. The full Archive verification was repeated before
recording compatibility report
`2026-07-17-windows-story-in-game-clean-exit`. The run's valid 39.46-second
stereo 44.1 kHz signed 16-bit wave output contained non-silent signal, but the
default Windows device was not heard and the known multi-context attribution
caveat still applies.

## Maintainer manual stopping point

On 2026-07-18, the maintainer launched Ricky through the app picker using the
clean `22e0bd2a4ec040cdda456aeaf7a3a72a4a80fb5d` release executable. The log
identifies the selected file as the exact canonical
`Ricky (v2.1) [Cracked].ipa`, records the expected bundle identity, and shows
that the new default options were loaded. A post-run hash check still matched
the Archive MD5, SHA-1, and repository SHA-256.

The maintainer manually confirmed audible output through the normal Windows
speaker path, jumping, defeating enemies, and restarting a level. They also
created and saved a custom level. The save alert and a newly written custom
level file corroborate creation, but selecting that level and pressing Play
crashed tapHLE. The terminal crash text was not captured, so the cause is not
yet localized.

This remains a three-star `in-game` result rather than four-star `playable`:
substantial functionality works, but the whole main game has not been shown
playable and custom-level playback has a major crash. The observation is kept
here rather than appended to the public database because the strict
report-worthy Archive verification was not run immediately before that manual
session. A future Ricky pass can make it an official appended report by
verifying first and reproducing the short manual checkpoint on an exact
committed build.

## Next compatibility frontier

If Ricky work resumes, start at the custom-level Play crash. Capture terminal
output and resolve its first emulator boundary before repeating broad gameplay.
After that, validate story completion/progression, save persistence across a
fresh process, the physical-controller path, and a longer memory trend for the
`0x40`-byte allocation quarantine. Audio should become `working` in the next
eligible report if normal-device audibility is reconfirmed.
