# Max Adventure Free compatibility work note

- Branch: `compat/max-adventure`.
- Canonical artifact: <https://archive.org/details/app-id-233>,
  `MaxAdventureFree 1.2.ipa`, `source: original`; size 18,985,524 bytes.
- Hashes:
  - MD5: `c5fee449c86d3e693f6445a8f6e8c37f`
  - SHA-1: `630c4639b663ae8ed477c4e37817e95298a06479`
  - SHA-256: `043ea5c352a1ac151ff6002c0cc8510357243f699980a480a0d2e098c70ac610`
- Embedded identity (`tapHLE --info`): display `MaxAdventureFree`, bundle
  `com.imangi.maxadventurefree`, version `1.2`, minimum OS `3.0`, iPhone +
  iPad. The Archive item is named only `app-id-233`, so `--info` is the only
  source of this app's identity.
- tapHLEdb: App 27, version 27, report 35 (2026-07-26, tapHLE `afcc4cf5`,
  1 star). See the caveat below on how to read that rating.

## This artifact is encrypted and cannot be tested

**Both slices have `cryptid = 1`.** The binary is still FairPlay-encrypted, so
tapHLE loads a slice and then executes ciphertext. The observed behaviour — the
last log line is `Loading armv7 slice for "MaxAdventureFree"`, then the process
dies with no Rust panic, no register dump and nothing on stderr under
`RUST_BACKTRACE=1` — is exactly what that produces.

**Report 35's 1-star rating describes an untestable artifact, not a tapHLE
limitation.** Nothing about this app has actually been evaluated.

### A hypothesis this refuted

Before checking `cryptid`, this app and JungleZuma were recorded as sharing one
Mach-O/dyld loader bug, on the reasoning that two unrelated apps failing at the
same point with the same absent panic was unlikely to be coincidence. That was
wrong: both are simply encrypted. The shared signature was real, the shared
cause was not the loader.

The durable lesson is now in `docs/debugging.md` under
"Check the binary is decrypted before blaming the emulator".

## Next step

Obtain a decrypted copy, or drop this app as a target. No emulator work is
worth doing against these bytes.
