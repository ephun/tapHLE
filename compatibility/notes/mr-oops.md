# Mr. Oops!! compatibility work note

- Branch: `compat/mr-oops`. Reusable fix graduated to `trunk`.
- Canonical artifact: <https://archive.org/details/iOSObscura>, file
  `iOS 4/jp.co.ponos.mroops/Mr.Oops!!-(jp.co.ponos.mroops)-1.2.2-(iOS_4.3)-828f2d26ba2b5e8b89df4776ca36c1e9.ipa`,
  `source: original`; size 17,474,141 bytes.
- Hashes:
  - MD5: `828f2d26ba2b5e8b89df4776ca36c1e9`
  - SHA-1: `4eb002dac78f30a4308fcd9aba80bde4de69160c`
  - SHA-256: `bced8e7e8e9cfd0d9c80264b0a6919746aed44b7530bbe58cd7e08c5fb43aa6d`
- Embedded identity: bundle `jp.co.ponos.mroops`, version `1.2.2`, minimum
  OS `4.3`. Same developer as Mr. AahH!!, but a much later build.
- tapHLEdb: App 18, version 18, report 26 (2026-07-26, tapHLE `8832a3e1`,
  ★☆☆☆☆).

## Current state: 1-star, no frame

The guest faults during startup with a null dereference:

```text
Attempted null-page access at 0x0 (0x4 bytes)
R2: 0x00000000   LR: 0x00017b5d   PC: 0x00017b82
```

The last thing logged before it is
`TODO: [(UIWebView*) loadRequest:(null)]`, so a null request reached UIWebView,
but that TODO returns harmlessly and is probably a symptom rather than the
cause.

## Rejected hypothesis: the missing ARC runtime

`_objc_retain` was an unresolved non-lazy symbol, which looked like a very
strong lead for a null-slot dereference in an ARC-compiled app (minimum OS 4.3
is well into the ARC era). tapHLE had **no** ARC entry points at all, so they
were implemented (`dd26fd62`) — a worthwhile addition regardless.

**It did not fix this app.** After the fix `_objc_retain` resolves, but the
fault is byte-for-byte identical: same PC `0x17b82`, same registers. So the
null being dereferenced is something else. Do not re-investigate ARC here.

## Still-unresolved non-lazy symbols, in priority order

Any of these is a candidate for the null slot, and all are cheap to add:

- `_kCFNumberNaN`, `_kCFNumberPositiveInfinity`, `_kCFNumberNegativeInfinity`
- `_NSURLAuthenticationMethodServerTrust`
- `_UIPasteboardChangedNotification`, `_UIPasteboardChangedTypesAddedKey`
- `___objc_personality_v0` (exception unwinding; present in several apps
  without apparently mattering)

## Next discriminator

Extract the executable and disassemble around `0x17b82` — it is a two
instruction window from `LR 0x17b5d`, so the call site is immediately
identifiable. Read which global the faulting load uses and match it against the
list above. That is one lookup and it decides the fix, rather than adding all
six constants speculatively.

## 2026-07-27: the nil is in the OAuth path, not in UIWebView

Still 1-star, still the same null dereference at `PC 0x17b82`. The earlier note
guessed that the preceding `[(UIWebView*) loadRequest:(null)]` was a symptom
rather than the cause. That was right, and tracing now says what the cause is.

The last messages before the fault, with `TAPHLE_TRACE_SELECTORS=all`:

```text
[OAMutableURLRequest (0x30013250) dealloc]
[nil ((null)) autorelease]
[OARequestParameter alloc] / initWithName:value:
[nil ((null)) key]
[nil ((null)) setParameters:]
[UIWebView loadRequest:]        <- the nil request finally arrives here
```

`OAMutableURLRequest` and `OARequestParameter` are **OAuthConsumer**, the
OAuth 1.0 library. The request object is constructed and then **deallocated**
before the parameters are attached, and `[nil key]` says the `OAConsumer` is
nil too. So the chain fails at the top — no consumer, therefore no signed
request — and a nil request is handed to the web view, after which the app
dereferences a null in its own code.

### What this means for the rating

This is the ad/analytics sign-in path. tapHLE has no network stack for it, and
the app's own offline handling is what runs. Whether the missing consumer is a
tapHLE gap or the app's correct behaviour with no network is **not yet
established** — and that is the question to answer first, before implementing
anything. Two cheap discriminators:

- Find what builds the `OAConsumer`. If it reads a key and secret out of a
  bundled plist, tapHLE failing to load that plist is a real gap and a fixable
  one.
- If instead the consumer comes from a server round-trip, this path cannot
  succeed offline and the bug is that the app does not survive its own failure
  — which tapHLE cannot fix from the outside, and which makes this a poor
  target until the rest of the app is reachable another way.

Do not start by implementing OAuth. Nothing here has shown that the app needs a
working OAuth exchange to reach its game.
