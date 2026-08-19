# SPYmouse HD compatibility work note

- Work branch; this investigation started from `compat/spy-mouse` at
  `fec8bcb1`.
- Local test artifact: `SPYmouseHD_v1.1.1_os32.ipa`; SHA-256
  `92b4712f4c15d17c83c237692126275519583568b115ddeeb69507669adaaeb5`.
- Artifact provenance: the maintainer confirmed this local IPA is the right
  target for this work. It is content-hash-verified against the original
  `SPYmouseHD_v1.1.1_os32.ipa` in
  `https://archive.org/details/ios_3_2_ipa`: MD5
  `329a4efcd51ca1b5005bbeda3ac49628`, SHA-1
  `9474d90a85215ece455c4046fd9e28bd456b7c05`, and locally calculated SHA-256
  `92b4712f4c15d17c83c237692126275519583568b115ddeeb69507669adaaeb5`.
- Availability check (2026-07-24): no current App Store listing was found for
  this exact EA bundle/version. Apple does list a separate modern app with a
  similar name by Dram Inc.; that is not a claim about the availability or
  legal status of this legacy EA build.
- Runtime milestone: on Windows, the `bddd7c91` release build of this exact
  artifact clears KVC's `dictionaryWithValuesForKeys:`, custom `NSDictionary`
  subclass key enumeration, `NSAssertionHandler`, `NSException`, `NSMachPort`,
  input-port registration, `dladdr`, `CCHmac`, `NSData` formatting, RFC-2396
  percent escaping, immutable dictionary values, guest `statfs`, and
  `CTTelephonyNetworkInfo`. It executes the game's offline ad/tracking failure
  handling, initializes its tracking database, displays the SPYmouse splash,
  and enters an active level after a tap. The level's HUD, mouse, cheese, and
  board remained visible and the process remained responsive 20 seconds later.
- Rating evidence: this is a three-star agent result: a gameplay loop starts
  and persists on the confirmed local IPA. The online-ad requests still fail
  offline as expected, and the runtime logs repeated missing-file warnings,
  but neither prevented the level from running.
- Current implementation: reusable Foundation KVC/dictionary, data, assertion,
  exception, Mach-port, and run-loop behavior; CommonCrypto HMAC-SHA1/MD5;
  a standard failed `dladdr` lookup; and a CoreTelephony device-information
  fallback which reports the documented no-cellular-provider result. The
  exception and input-port fallbacks are explicitly bounded until Objective-C
  unwinding and generic port delivery are implemented.
- Current frontier: the three-star milestone is complete. A human may assess
  the game for four stars; do not change this agent rating without a new exact
  artifact run.
- Checks passed: `cargo metadata --no-deps --format-version 1`,
  `cargo fmt --all -- --check`, `git diff --check`, and `cargo build --release`.
  The exact IPA was launched on Windows after that release build and reached a
  persistent active level. `cargo test --workspace --lib` was attempted but
  failed before tests because the debug target refers to an unavailable former
  worktree, Boost is absent, and CMake is not on the debug-build PATH. The
  TestApp suite likewise remains unavailable without its custom SDK/LLVM.
- Compatibility database: submitted the three-star `source_type: agent` result
  for `bddd7c91` on 2026-07-24 after the content-hash verification. It created
  app 6, version 6, and report 10, all pending maintainer moderation.

## 2026-07-27: REGRESSION to two stars, tapHLE `53bc1f24`

**The three-star result above no longer reproduces.** The app now reaches the EA
splash logo and aborts before any level:

```text
Object (class "_tapHLE_NSArray") does not respond to selector "serialize"!
```

Filed as report 46 (★★☆☆☆). A regression is a result, so it is recorded rather
than left as a stale three-star claim.

### Cause

The app bundles `AS_NSArrayJSONSerializable`, a **guest subclass of NSArray**.
tapHLE hands a guest subclass of NSArray a plain `_tapHLE_NSArray`, so the
object loses its own class and the `serialize` its subclass defines is not
found.

Until 2026-07-27 the same path hit `assert!(this == NSArray)` in
`+[NSArray allocWithZone:]` instead — also fatal, one step earlier. So **the
regression is older than that change**, which only moved where it dies. It was
confirmed by bisect: the assertion fires identically on `cc492376` and
`b1de9e9e`. The commit that first broke it has not been identified.

### Why it is not fixed yet

The real fix is to let a guest subclass of NSArray inherit tapHLE's concrete
implementation, since the array primitives (`count`, `objectAtIndex:`) live on
`_tapHLE_NSArray` and not on `NSArray`. An instance of the subclass allocated
today would have no storage at all. Two ways:

1. **Re-parent at class registration.** When a guest class's superclass
   resolves to `NSArray`, substitute `_tapHLE_NSArray`. `ClassHostObject::
   from_bin` is the place, and tapHLE already has both class substitution and
   ivar-offset reconciliation. The obstacle is mechanical: `from_bin` holds
   `&Mem`, and `get_known_class` wants `&mut Mem`.
2. **Make `NSArray` itself concrete**, moving the primitives up from
   `_tapHLE_NSArray`. Larger, and it would retire the sibling-class trap that
   has cost this codebase repeated rebuild cycles.

This is the same shape as the `_tapHLE_NSString` / `_tapHLE_NSMutableString`
problem, so whichever is chosen should be applied to strings and dictionaries
too rather than to arrays alone.

### What still works

Startup, the offline ad/tracking failure handling, and the splash. The window
stays up for roughly forty seconds before the abort, so a sweep that captures
early sees a healthy-looking app — which is how this went unnoticed. A
regression check for this app must drive it to a level, not merely confirm the
process is alive.

## 2026-07-27: three stars restored, tapHLE `b953ef9c`

The regression above is **fixed**. A tap on the splash enters a level and the
game runs: the mouse, the cheese, the drawn route and the HUD all render, and
two captures eight seconds apart differ, so the loop is live rather than a
frozen frame. Filed as report 47 (★★★☆☆), superseding report 46.

The fix is the first of the two options listed above, and it is on `trunk`:
class registration now re-parents a guest class whose superclass is an abstract
Foundation cluster class onto tapHLE's concrete implementation of it. The
metaclass is re-parented alongside the class, which is the half that matters for
`+alloc` — without it, allocation still resolves to the abstract class's and
returns an object of the wrong class.

It is applied to the array, string and dictionary clusters together rather than
to arrays alone, since leaving the others would have left the same trap armed.
The substitution is skipped with a warning if the abstract and concrete classes
ever differ in instance size, because that would move the subclass's ivars.

### The lesson that stands regardless

This was found by a sweep, but only because the sweep was *followed up*: the
early capture showed a healthy-looking splash, and the app did not abort until
roughly forty seconds in. A liveness check is not a compatibility check for an
app whose failure is delayed. The regression check for this app drives it to a
level.
