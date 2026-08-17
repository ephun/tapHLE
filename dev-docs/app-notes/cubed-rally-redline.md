# Cubed Rally Redline compatibility work note

- Branch and last pushed commit: `compat/cubed-rally-redline`, see below for the
  tested revision.
- Artifact: maintainer's local copy, filename
  `Cubed Rally Redline (v1.32) [Decrypted].ipa`,
  SHA-256 `837ED1C107602EC6738CBD29DFE53BCD15F6DD57A429D7F9F88A6BDAD3F7DE87`.
  Not Archive-backed; no public provenance recorded.
- Identity, read from `tapHLE --info` (not from the filename): display name
  `Redline`, bundle identifier `com.nocanwin.redline`, bundle version `1.32`,
  minimum OS version `5.0`, armv7, iPhone + iPad, landscape only.
- Highest clean committed milestone: **three stars — the game is playable and
  renders correctly**. Menus, car select, the voxel landscape, the dirt road
  and its grass verges, water, fences, clouds, particles and the HUD all draw
  as they should. Two full races were played end to end in one session —
  `Play`, `Game`, `Player: Dead`, `EndTimed:Game`, `NewGame`, `Game`,
  `Player: Dead`, `EndTimed:Game` — with zero panics and zero aborts.
- Tilt steering does not work (Core Motion is a no-op) and some compressed
  audio is silent (`AudioQueueOfflineRender` is unimplemented). Neither stops
  play: the on-screen left/right controls steer fine.
- **The two-star boundary was never reported.** This app reached menus well
  before it reached gameplay, and no report was filed at the time, so the
  database has no two-star snapshot for it and its first entry is the
  three-star one. That gap is not recoverable — a report asserts a rating was
  made against a revision when it was run — so it is recorded here instead.
  File every boundary as it is crossed; see `compatibility/README.md`.

## What this app is

A Unity 4.0.1f2 game, built against the iOS 6.0 SDK. That makes it much newer
than tapHLE's usual targets and the first Unity title to run, so most of what
was fixed for it is engine-shaped rather than game-shaped: the Mono runtime, its
allocator, and Core Foundation string and data bridging. `--info` warns that the
app wants iOS 5.0 and tapHLE supports 4.0 and earlier; that warning is not fatal
and did not need addressing.

`cryptid` is 0, so the binary is genuinely decrypted and load failures would be
real results rather than an artefact of a bad dump.

## Click map

Recorded as a replayable clickmap in
`dev-docs/clickmaps/cubed-rally-redline.json`, verified end to end on
`58e8c1a6`. Replay it with `dev-scripts/clickmap.ps1` before exploring.

Launch options: none (no entry in `tapHLE_default_options.txt`). Window client
area 480x320, landscape, matching the app's own resolution 1:1.

Wait about 55 s after launch before the first tap — the Mono runtime, scene load
and asset unload take that long.

**Press duration matters: use ~300 ms.** A 90 ms press is not seen at all, and
that produced a convincing false negative — the title screen animates on its own
for a few seconds after load, so a frame comparison right after a missed tap
still shows a change. Confirm the screen is static first, then tap.

| From | Client tap | To |
| --- | --- | --- |
| Title / mode select | (242, 141) | Car select |
| Car select | (247, 264) | Controls screen |
| Controls screen | (239, 287) | Race |

## Proven

- Unity engine initialises, Mono loads every assembly including
  `Assembly-CSharp.dll`, and scenes load (4711 objects for the race scene).
- `applicationDidBecomeActive()` fires and the Core Animation compositor
  presents.
- Gameplay is real, not a still frame: `stdout` records `NoMetric:Play`, then
  `Player: Dead`, then `NoMetric:EndTimed:Game`, and OS-level screenshots hash
  differently between samples during the race.
- These helpers use the base AAPCS convention (operands and results in core
  registers, not VFP), read off `___fixdfdi` and `___floatdidf` call sites in
  this binary rather than assumed.

## Rejected

- "The soft-float helpers return in `d0` because iOS is AAPCS-VFP." Disproved by
  disassembly: the app does `vmov r0, r1, d16` immediately before calling
  `___fixdfdi` and reads a `___floatdidf` result from `r0`/`r1`.
- "The app freezes because it is waiting for input." It was spinning on an
  `mmap` that could not be satisfied; frames were byte-identical for 30 s.
- "The first tap did not land because touch routing is broken." The press was
  simply too short.


## Underlying systems this app uses that tapHLE does not implement

Measured, not guessed. Two sources, and they answer different questions:

- **Imported but unprovided** — `tapHLE --dump=linking-info` on the exact
  artifact: 102 lazy symbols with no `linked_to`, and 27 Objective-C classes
  tapHLE reports as `unimplemented`. An entry here is a gap in tapHLE's API
  surface; it does not prove the game calls it.
- **Actually reached** — the `TODO:`/`Warning:` lines a real windowed run
  emits while playing to a race result. These are the gaps the game hits.

Do not read the 930 selectors the dump lists as unimplemented as gaps. Almost
all are the app's own (Chartboost and Prime31 ad-SDK) selectors, which have no
host implementation because they are guest code.

### Reached during play, in rough order of how much they cost the game

1. **Offline audio rendering.** `AudioQueueOfflineRender` is unimplemented, so
   `AudioQueueSetOfflineRenderFormat` fails. This is how Unity decodes
   compressed clips, so those sounds are dropped. Fails cleanly; the game runs.
2. **Core Motion.** `CMMotionManager` is reached twice per launch. Unity reads
   acceleration through the block-based
   `startAccelerometerUpdatesToQueue:withHandler:`, which is a no-op, so tilt
   steering does not work. The game is still playable with the on-screen
   left/right controls.
3. **Audio session properties.** Three `AudioSessionSetProperty` and three
   `AudioSessionAddPropertyListener` calls are TODOs, so the app cannot observe
   route changes or interruptions.
4. **Analytics and ads.** `Flurry` is the only faked class actually messaged
   (`logEvent:`, `logEvent:timed:`, `logEvent:withParameters:`,
   `setSessionReportsOnPauseEnabled:`). Chartboost and Prime31 RestKit are
   present in the binary but their network paths produce nothing, since
   `NSURLConnection` is a stub and `NSJSONSerialization` is unimplemented — the
   app falls back to its bundled JSONKit, which is what exposed the
   guest-allocated-collection defect.
5. **`__cxa_atexit` (476 calls) and `atexit`.** Registered destructors are
   never run. Harmless while the app is alive.
6. **`readlink` (256 calls), `stat`/`fstat` (mostly unimplemented), `ferror`,
   `sigaction` (7 signals), `sigprocmask`, `signal`, `mprotect`,
   `pthread_attr_setschedpolicy`/`setschedparam`, `pthread_detach`.** Mono
   probes all of these; none is fatal.
7. **UIKit trimmings** actually touched: `UIViewController` properties,
   `UITextField` border/clear-button styles, `setExclusiveTouch:`,
   `isGeneratingDeviceOrientationNotifications`, and the
   "existing view hidden by another view" appearance case.
8. **`AVCaptureSessionPreset*`** constants are unresolved non-lazy symbols, so
   the camera path would fault if it were ever entered. It is not entered in
   normal play.

### Imported but unprovided, grouped by system

Not observed to be called in a normal play session; listed so the next agent
knows what is missing rather than re-deriving it.

- **Game Center (15 classes):** `GKAchievement`,
  `GKAchievementDescription`, `GKAchievementViewController`,
  `GKFriendRequestComposeViewController`, `GKLeaderboardViewController`,
  `GKMatchRequest`, `GKMatchmaker`, `GKMatchmakerViewController`, `GKPlayer`,
  `GKTurnBasedEventHandler`, `GKTurnBasedMatch`,
  `GKTurnBasedMatchmakerViewController`, `GKTurnBasedParticipant`,
  `GKVoiceChat`, `GKVoiceChatService`.
- **Camera capture:** `AVCaptureSession`, `AVCaptureDevice`,
  `AVCaptureDeviceInput`, `AVCaptureVideoDataOutput`, plus
  `CMSampleBufferGetImageBuffer`, `CMTimeMake` and the six `CVPixelBuffer*`
  entry points.
- **Networking and serialization:** `NSHTTPURLResponse`, `NSURLCredential`,
  `NSJSONSerialization`, `NSPredicate`,
  `CFURLCreateStringByReplacingPercentEscapesUsingEncoding`.
- **Objective-C exceptions and runtime introspection:**
  `objc_begin_catch`, `objc_end_catch`, `objc_exception_rethrow`,
  `objc_enumerationMutation`, `class_getInstanceVariable`,
  `class_getIvarLayout`, `ivar_getName`, `ivar_getOffset`, `object_setIvar`,
  `protocol_getMethodDescription`.
- **Mach-O/dyld introspection:** `_dyld_image_count`,
  `_dyld_get_image_header`, `_dyld_get_image_name`,
  `_dyld_register_func_for_add_image`, `getsectbynamefromheader`.
- **Drawing:** `UIBezierPath`, `UIPinchGestureRecognizer`,
  `UIImagePNGRepresentation`, `UIRectFill`, `CGGradientCreateWithColors`,
  `CGContextDrawLinearGradient`, `CGContextDrawRadialGradient`,
  `CAKeyframeAnimation`, `CATransform3DRotate/Scale/Translate`.
- **Store:** `SKMutablePayment`.
- **POSIX process and IPC:** `fork`, `execv`, `execve`, `pipe`, `poll`,
  `kill`, `raise`, `dup2`, `getuid`, `geteuid`, `getpwnam_r`, `getpwuid_r`,
  `getgrnam_r`, `getgrgid_r`, `getrusage`, `getpriority`, `setpriority`,
  `setitimer`, `sched_get_priority_min/max`, `pthread_cancel`, `pthread_kill`,
  `chmod`, `rmdir`, `utime`, `tcgetattr`, `tcsetattr`, `tcflush`.
- **Sockets:** `getnameinfo`, `getpeername`, `getprotobyname`, `inet_ntoa`,
  `recvmsg`, `sendmsg`, `freeifaddrs`, `gai_strerror`.
- **Other libc:** `__assert_rtn`, `__snprintf_chk`, `__vsnprintf_chk`,
  `strtoll`, `bcopy`, `nl_langinfo`, `iconv`/`iconv_open`/`iconv_close`,
  `sys_icache_invalidate`, `dlerror`, `_exit`.
- **Apple System Log:** `asl_new`, `asl_get`, `asl_key`, `asl_search`,
  `aslresponse_next`, `aslresponse_free`.
- **CommonCrypto:** `CC_SHA1_Init`, `CC_SHA1_Update`, `CC_SHA1_Final`.
- **Core Foundation:** `CFArrayCreate`, `CFArrayGetValues`,
  `CFDictionaryCreate`, `CFUUIDGetUUIDBytes`, and the four `CFBitVector*`
  entry points.
- **Audio:** `AudioUnitRender`.
- **GLES 2 introspection:** `glGetActiveAttrib`, `glGetActiveUniform`,
  `glGetShaderSource`. The game runs its GLES 1.1 path, so these are unused.

**None of this explains the rendering defect.** Every gap above is a missing
API; the render failure is in code tapHLE does implement. Keep the two apart.

## The rendering defect and its cause (solved 2026-08-04)

**Root cause: tapHLE advertised `GL_OES_vertex_array_object` without
implementing it.** The GLES1-on-GL2 backend inherits the no-op defaults from
the `GLES` trait — `BindVertexArrayOES` does nothing, `GenVertexArraysOES`
returns colliding names. Unity checks the extension string, believed it, and
so configured each mesh's array pointers once into a vertex array object and
thereafter only bound the object before drawing. With binding a no-op, every
such batch drew with whichever pointers the previous draw had left behind.

That is why the track was drawn with its own vertex positions in place of its
texture coordinates — sampling one white corner of the sprite atlas, which
modulated by a half-strength primary colour gives exactly the (128,128,128)
flat grey that covered the world. It is also why menu artwork was overdrawn by
other objects' geometry.

Removing the extension from the reported string fixes it: the game falls back
to setting its array pointers per draw, which always worked. A second defect
surfaced once the game ran long enough to collect garbage — `mprotect`
returned -1, so Boehm printed `Mprotect remapping failed` and aborted. Both
fixes are on trunk.

The measurements that led there are kept below, because they are what ruled
out everything else.

### How it was measured

Everything written before 2026-08-03 about textures, combiners and mip
selection was observed through the double-rotation presentation bug and is
worthless. The 2026-08-03 conclusion — "seven world batches rasterise zero
pixels, the geometry is placed where the camera cannot see it" — is **also
wrong**, and is corrected below. It came from computing NDC by hand for one
vertex read out of a shared scratch buffer at the wrong offset.

This section is measured with a probe that read back the framebuffer before
and after **every** draw of a race frame, alongside the matrices, the GL state,
and the vertex/texcoord data read through the very array pointers the driver
uses (`glGetPointerv`, not the probe's own bookkeeping).

### What the defect actually is

A race frame is 13–16 draws. Every one of them rasterises. Nothing is culled
away and nothing lands off-screen by mistake. The frame is:

| what | indices | texture | result |
| --- | --- | --- | --- |
| terrain chunks | 228–480 each | 103 | 0–6 pixels each |
| **ground plane** | **1050** | **103** | **~80 000 px, flat (128,128,128)** |
| car | 1020 | 99 | correct |
| clouds | 18–36 | 97 | correct |
| HUD, text, particles | 6–546 | 35/65/67 | correct |

So the world **is** drawn. The screen-covering grey sheet the maintainer sees
as "shredded diagonal streaks" is one 1050-index ground plane resolving to a
single flat colour, and the voxel terrain chunks are each a handful of pixels.

The projection is a correct 480x320 orthographic matrix, the modelview is a
constant proper rotation, and the camera position recovered from it
(`-Rᵀt`) tracks the car sensibly: y fixed at 19.71, z advancing about 0.3
units a frame while the car drives. **The camera and the geometry transform
are fine.** Do not look there again.

### Why the ground plane is flat grey

- Texture 103 is a 1024x1024 `GL_RGB`/`GL_UNSIGNED_BYTE` sprite atlas with a
  guest-supplied 11-level mip chain, `MIN_FILTER = NEAREST_MIPMAP_NEAREST`,
  `MAG_FILTER = NEAREST`, wrap `REPEAT` on both axes.
- The bytes the guest hands `glTexImage2D` are **correct**. They were dumped
  and viewed: a proper atlas, art packed into a strip at one end, the rest
  white. Levels 0–10 are all sane; the 1x1 level is (239,238,235).
- The ground plane's texture coordinates, read through GL's own array pointer,
  span **u ∈ [-10.4, +21.4], v ∈ [-1.0, +1.0]** — while every other draw in
  the frame is inside 0..1. The u range *slides* as the car drives (20.19,
  20.60, 21.44 on three consecutive frames) but keeps a roughly constant span
  of about 31. That is a scrolling, tiling strip.
- Tiling a mostly-white atlas 31 times across the screen makes the minification
  derivative enormous, so the sampler lands on the top mip levels, which are
  near-white. Near-white texel × a ~0.5 primary colour is exactly the observed
  (128,128,128).
- Proof: forcing every `*_MIPMAP_*` minification filter down to `GL_NEAREST`
  makes texture detail reappear on that plane (a tiled, multicoloured band
  instead of flat grey). The grey is texture-sampling collapse, not geometry.

The state at that draw: `TEXTURE_ENV_MODE = GL_COMBINE`, `GL_LIGHTING`
enabled, blending off, depth test and depth write on, culling on, texture
matrix identity, `ACTIVE_TEXTURE = CLIENT_ACTIVE_TEXTURE = GL_TEXTURE0`.

### Ruled out, with the evidence

- **The texture matrix.** The guest touches it five times in a whole session,
  always `glLoadIdentity` on an already-identity matrix. It never scales UVs
  that way, so `present_frame` clobbering it could not be the cause. (Its
  push/pop in `present_renderbuffer` is symmetric anyway.)
- **`fmod`/`fmodf`.** Rust's `%` on floats is C `fmod` semantics; the
  implementations in `src/libc/math.rs` are correct.
- **The soft-float helpers.** `src/libc/compiler_rt.rs` is correct for every
  finite value, and the marshalling is pinned by tests.
- **`mach_absolute_time`/`mach_timebase_info`.** Nanoseconds with a 1/1
  timebase; correct.
- **The buffer-mapping path.** `glMapBufferOES` is never called by this app.
- **`--landscape-native`**, which is on and fixed a *different*, earlier defect.

### The 228–480 index batches are particles, not terrain

Their predicted window bounding boxes, computed from the guest's own vertex
and index data, are 18x15, 30x9 and 18x14 pixels — and they paint exactly
that. 396 indices is 66 quads: a puff of dust beside the car, not a chunk of
world. Nothing is collapsing them.

The consequence is the important part. **The voxel terrain is never submitted
at all.** A race frame contains a ground ribbon, particles, the car, cloud
sprites and the HUD, and that is the whole list. One run showed the game's own
`LOADING` caption still on screen well into a race.

### What the flat ribbon is

1050 indices, 350 triangles, predicted extent 1297x321 pixels, texture
coordinates running `u ∈ [-10.4, +21.4]` along its length and `v ∈ [-1, +1]`
across it, sliding as the car drives. That is a road ribbon meant to tile a
repeating road texture — and the texture bound to it is the sprite atlas.

### Also ruled out, 2026-08-04

- **A missing asset file.** With `open()` now naming the file it failed on,
  every failure in a full session is a Unity `.resS`/`.resG`/`.res` streaming
  sidecar or a mono GAC `policy.2.0.*` stub. All are optional probes with
  fallbacks. Nothing the game needs is missing from the FS.
- **The texture-environment combiner.** `TEX_ENV_PARAMS` in
  `src/gles/gles1_on_gl2.rs` forwards `COMBINE_RGB`, `COMBINE_ALPHA`, the
  `SRC*` and the `OPERAND*` parameters. `GL_COMBINE` is not being dropped.
- **Vertex colours.** Drawing the whole race with `GL_TEXTURE_2D` forced off
  renders every object white or grey. The geometry carries no colour of its
  own; all colour comes from the texture. So the ribbon's appearance depends
  entirely on where it samples, which is the open question.

### What actually found it

Every measurement above was of the *symptom*, and each one narrowed the search
without reaching the cause, because they all assumed the guest had asked for
what tapHLE was drawing. The step that broke it open was noticing that the
broken draws issued **no array-setup calls at all** in their own frame — their
pointers had been configured in some earlier frame — and then asking how a
draw can inherit array state. That is what vertex array objects are for, and
`tapHLE --dump=linking-info` shows the app importing
`_glGenVertexArraysOES`, `_glBindVertexArrayOES` and `_glDeleteVertexArraysOES`,
all `linked_to: "host"`.

The transferable lesson: when a draw's arrays are wrong and the guest did not
set them that frame, suspect the state-object mechanism before suspecting the
data. And check what tapHLE *claims* to support, not only what it implements —
an extension string is an API surface, and this one was lying.

### A note on the numbers above

The `u ∈ [-10.4, +21.4]` texture coordinates were never the guest's intent.
They were the vertex positions of a different object, read through a texture
coordinate pointer that belonged to a batch drawn earlier. Anyone re-reading
this section should treat those figures as a description of the corruption,
not of the road.
