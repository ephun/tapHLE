# Parachute Panic HD compatibility work note

- Branch and starting commit: `compat/parachute-panic-hd` from `186e18ab`.
- Canonical artifact: `https://archive.org/details/iOSObscura`, file
  `iPhoneOS 3/com.FDGMobileGamesGbR.ParachutePanic/ParaPanicHD-(com.FDGMobileGamesGbR.ParachutePanic)-1.9.1-(iOS_3.0)-2f707eb5e2c7ce80b3b76984a7ef46f9.ipa`;
  MD5 `2f707eb5e2c7ce80b3b76984a7ef46f9`, SHA-1
  `da09acc0f7018d1e2daeedb8d1eea831b97d750a`, SHA-256
  `6834b6c15a2b57f8ba51aa0599c0eadcf8deb531468de3911fae04f387613a31`,
  size 19,722,854 bytes. MD5/SHA-1/size match the live Archive metadata
  (source=original). Local Windows-safe filename `ParachutePanicHD 1.9.1.ipa`
  maps to the Archive filename above (use `--archive-filename` when reporting).
  A separate 1.6 build also exists in the item; the maintainer URL is 1.9.1.
- Embedded identity: display `ParaPanicHD`, bundle
  `com.FDGMobileGamesGbR.ParachutePanic`, version `1.9.1`, minimum OS `3.0`,
  universal iPhone/iPad.
- Availability: not re-checked against Apple lookup yet.

## Highest milestone: 3-star (In game), tapHLE `a85548d6`

Reproduced on a clean committed release build (window title
`ParaPanicHD (tapHLE a85548d6)`, no `-dirty`). Filed as report 49.

The title screen, the "What's New?" dialog, the main menu (New Game,
Challenges, High Scores, Themes, Extras, How To Play, News) and the
theme/difficulty screen all render, and **Easy starts a round**: the plane
crosses the top, a parachutist falls, the boat sits on the water, and the score
and lives HUD is drawn. Three captures six seconds apart were three distinct
images, checked by SHA-256 rather than by eye.

Not assessed past the first round, so this is three stars and not more — and
four and five require human testing in any case.

### Click map

Window 320x480. Allow ~32 s for the title screen.

1. Title, with the "What's New?" dialog -> `(160, 350)` OK. Allow 12 s.
2. Main menu -> `(160, 165)` New Game. Allow 16 s.
3. Theme / difficulty -> `(75, 395)` Easy -> the round starts after ~18 s.

### What it took

Five general gaps, all on `trunk` and none specific to this app:

1. `-[NSNumberFormatter setMinusSign:]`. The app's `CreditsFormatter` is a
   guest subclass of NSNumberFormatter and died on it before drawing anything.
2. `glGetTexParameteriv`, which existed on no backend.
3. Type encoding `I` in `NSMethodSignature`, then `I` again in `NSInvocation`.
   Both are now handled as part of the **full** scalar set rather than one
   character at a time — `I` was missing only because nothing had needed it,
   and the next app would have found the next hole.
4. `NSApplicationSupportDirectory` in
   `NSSearchPathForDirectoriesInDomains()`.
5. `-[NSFileManager copyItemAtPath:toPath:error:]` asserted that the caller had
   *not* passed an `NSError**`, which is backwards: an app that passes one is
   asking to be told what went wrong.

The earlier note said startup crashed inside the bundled Scoreloop SDK. That
was where it stopped, but none of the five fixes above is Scoreloop-specific —
the SDK was simply the first code to exercise them.

## Fixes made (general; graduate to trunk)

Each was the exact next startup blocker; all are reusable beyond this game:
1. `+[NSBundle bundleWithIdentifier:]` and `+[NSBundle bundleWithPath:]`.
2. `-[NSData subdataWithRange:]`.
3. CommonCrypto `CC_SHA224/256/384/512` (via the `sha2` crate).
4. Minimal KVO on NSObject: `addObserver:forKeyPath:options:context:` and the
   two `removeObserver:` forms (accepted, no notifications delivered).
5. `-[NSProcessInfo processIdentifier]` (reuses libc `getpid`).
6. Foundation/Security string constants that were unhandled non-lazy symbols and
   crashed on dereference: `NSHTTPCookieName/Value/Domain/Path`,
   `NSUnderlyingErrorKey`, and keychain keys `kSecReturnData/Attributes/`
   `PersistentRef`, `kSecValueData`.

## Frontier (2026-07-26): still no frame; parked as a poor value target

The keychain frontier below is **cleared**. Startup now gets past Scoreloop's
keychain query and several steps beyond it, but has still never drawn a frame.
Work here is paused by agreement with the maintainer in favour of the other
apps on the target list; everything found on the way is general and already on
`trunk`.

### Cleared since the keychain frontier

1. A real Security framework: the `kSec*` query constants plus working
   `SecItemAdd` / `SecItemCopyMatching` / `SecItemUpdate` / `SecItemDelete`
   over an in-memory generic-password store. (`744fa375`)
2. `-[NSOperationQueue setSuspended:]` / `-isSuspended`, with a pending list so
   operations added while suspended run in order on resume. (`744fa375`)
3. `NSNumberFormatter`, which did not exist at all. (`aa10bf46`)

### Why it is parked

Scoreloop subclasses `NSNumberFormatter` as `CreditsFormatter` and configures
it exhaustively. Each missing setter aborts the app, and each costs a full
release rebuild to discover:

`setCurrencyCode:` -> `setPositivePrefix:` -> `setMultiplier:` ->
`setPaddingPosition:` -> `setMinusSign:` -> ...

Enumerating the binary's `__objc_selrefs` up front (the technique that worked on
Glass Tower 3) did **not** bound this: `setMinusSign:` is reached through a
selector the sweep did not surface, so the real surface is larger than a static
list of formatter-shaped names predicts. The remaining set is the
`NSNumberFormatter` symbol properties — minus sign, plus sign, percent symbol,
zero symbol, nil symbol, exponent symbol, and so on.

That work is all shallow and mechanical, but it is a long tail, and behind it
sit further Scoreloop subsystems (networking, the request queue) before
anything renders. Six distinct subsystems have been implemented for this app so
far and it has not reached a first frame, which is a poor return compared with
apps that have not been tried at all.

### If resumed, do this

Implement the whole `NSNumberFormatter` symbol-property block in one pass —
`minusSign`, `plusSign`, `percentSymbol`, `perMillSymbol`, `zeroSymbol`,
`nilSymbol`, `notANumberSymbol`, `positiveInfinitySymbol`,
`negativeInfinitySymbol`, `exponentSymbol`, `currencyGroupingSeparator`,
`currencyDecimalSeparator`, `internationalCurrencySymbol` — as stored
properties, several of which genuinely affect output and should be wired into
`-stringFromNumber:` alongside the affixes already there. Only then resume the
crash-to-crash loop, and re-assess after the *next* subsystem boundary rather
than continuing indefinitely.

## Checks run

- Artifact hash verification vs live metadata: pass; SHA-256 recorded above.
- `cargo fmt --all -- --check`; `cargo test --workspace --lib`.
