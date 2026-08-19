# JungleZuma compatibility work note

- Branch: `compat/jungle-zuma`.
- Canonical artifact:
  <https://archive.org/download/ios-ipa-collection/JungleZuma%201.0.ipa>
  (identifier `ios-ipa-collection`), `source: original`; size 7,872,375 bytes.
- Hashes:
  - MD5: `0dafe987c2f4fe2a6ab6d9eb3fc12639`
  - SHA-1: `21d4a5689e9667b050cf84556dbda42d7d1aadd7`
  - SHA-256: `b40fd465fc29ed2e73dd8584d428f4fb3804faec2d4385e10a0a735593c44e2b`
- Embedded identity (`tapHLE --info`): display name `Zuma_HD`, bundle
  `com.szteamtop3.Zuma`, version `1.0`, minimum OS `3.0`, iPhone device family.
  Note the mismatch: the bundle is named `Zuma_HD.app` and ships 720/800/854-wide
  artwork, but `UIDeviceFamily` says iPhone, so tapHLE picks a 320x480 screen.
- tapHLEdb: no report. **Not rated** — it never reaches a screen.

## CORRECTION: the artifact is encrypted, so it cannot be tested

**`cryptid = 1`.** This IPA's `__TEXT` is still FairPlay-encrypted, so tapHLE
loads the slice and then executes ciphertext. Everything below was written
before that was checked and treats the failure as an emulator bug; it is not.

The `LeeroyJenkins` marker noted below shows a cracking tool ran, but it did not
decrypt the binary. Report 24 (1 star) therefore describes an untestable
artifact rather than a tapHLE limitation, and should be read that way.

To make progress, a **decrypted** copy of this app is needed. Do not
investigate the loader for this app.

## Original (superseded) analysis: hangs during app loading

On tapHLE `2c3c2dcf` the process starts, prints the bundle info, creates the
GLES1-on-GL2 context, logs the driver, and then **hangs indefinitely**. Run
under a 40 s timeout it is killed (exit 124); it does not panic and does not
exit on its own.

The decisive detail is what is *missing*: there is no
`tapHLE::mach_o: Loading <arch> slice for "Zuma_HD"` line, which every working
app prints immediately after the driver info. So execution never reaches guest
code — it is stuck in bundle or Mach-O loading, before dyld.

## Ruled out

- **Not a malformed or unusual archive.** The IPA has only 62 entries and an
  ordinary `Payload/Zuma_HD.app/...` layout, so this is not a pathological
  directory scan.
- **Not a Rust panic.** Nothing is printed to stderr and no backtrace appears
  with `RUST_BACKTRACE=1`; the process simply stops making progress.

## Lead worth checking first

The app root contains a file named `LeeroyJenkins` alongside the executable.
That is the marker left by the Clutch cracking tool, so this is a cracked
build rather than a clean decryption. A cracked Mach-O can carry an
inconsistent load-command table or a still-set `cryptid` with decrypted
contents. An infinite loop in tapHLE's Mach-O load-command walk on such a
binary would produce exactly this symptom, and would be a robustness bug worth
fixing regardless of this app: a malformed input should be rejected with a clear
error, never hang.

## Next discriminator

Attach a debugger, or add a bounded iteration guard plus a `log_dbg!` per load
command in `src/mach_o.rs`, and run again to see which command it stops on.
That single observation distinguishes "infinite loop on a malformed load
command" from "waiting on something else entirely", and decides whether the fix
belongs in the Mach-O parser or elsewhere.
