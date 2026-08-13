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
- tapHLEdb: App 18, version 18. Report 26 (2026-07-26, tapHLE `8832a3e1`,
  ★☆☆☆☆); report 67 (2026-08-12, tapHLE `e0645ce4`, ★★☆☆☆).

## 2026-08-12: two stars. The title screen, and a stage that starts

**Everything below this section is history.** It describes an app that faulted
during startup and never presented a frame. That is no longer what happens.
Read it for the reasoning, not for the current state.

Identity, read from `--info` and not from the filename: `Mr.Oops!!`,
`jp.co.ponos.mroops`, version `1.2.2`, minimum OS `4.3`, device family
iPhone/iPad. Local SHA-256 of the tested bytes:
`bced8e7e8e9cfd0d9c80264b0a6919746aed44b7530bbe58cd7e08c5fb43aa6d`, matching the
value recorded above.

Measured on `e0645ce4`, a clean worktree, with an OS-level `PrintWindow`
screenshot: the title screen comes up — logo art, the animating character, the
menu bars and the three small buttons — and is still live and animating at 45
seconds. Reported as tapHLEdb report 67 (2026-08-12, tapHLE `e0645ce4`,
★★☆☆☆), against the existing app 18 and version 18.

### What the null dereference actually was

The note below narrowed the fault to an unbound non-lazy data import and listed
`_UIPasteboardChangedNotification` among six candidates. It is that one.
`dev-scripts/nl-symbol-at.py` resolves the slot the faulting instruction loads
from:

```text
python dev-scripts/nl-symbol-at.py <ipa> 0xC85D8
0xc85d8 is in __DATA,__nl_symbol_ptr
SYMBOL: _UIPasteboardChangedNotification
```

The instruction pair at `0x17b7c`/`0x17b82` is the ordinary "load the GOT slot,
then load the global through it" sequence, so a slot left at zero faults on the
second load. The four `UIPasteboard` notification names are now exported from
`src/frameworks/uikit/ui_pasteboard.rs`.

### The OAuth path was a tapHLE bug after all

The section below asks whether the nil `OAConsumer` is a tapHLE gap or the app
behaving correctly offline, and says to answer that before implementing
anything. It was a tapHLE gap, and not in OAuth: `-[NSURLRequest
initWithURL:cachePolicy:timeoutInterval:]` returned nil whenever network access
was off, releasing itself on the way out. That is what the earlier trace saw as
`[OAMutableURLRequest dealloc]` before the parameters were attached — not the
app dropping its own request.

Requests are now built normally and `NSURLConnection` reports
`NSURLErrorNotConnectedToInternet` asynchronously instead of returning nil, the
way a device in airplane mode behaves. The Twitter request now carries its real
URL. Do not implement OAuth: nothing here needs a working exchange.

### Frontier: a stage starts but does not persist

Pressing the upper menu bar starts the game. The click map, in client
coordinates of the 480x320 window:

1. Launch, wait about 15 seconds for the title screen.
2. Click `(375, 179)` — the upper of the two dark bars on the right. It
   highlights purple.

The app then raises a one-button alert, `【First Mission】Clear 20waves to
unlock "Iron Cannons"!`, and on `e0645ce4` that alert reports its only button as
pressed rather than reporting no button, so the app continues. Stage 1,
"Rolling Stones", draws its title and the player on a grid playfield.

**It then returns to the title screen after about five seconds with no input.**
That is why this is two stars and not three: the loop starts and does not
persist. The playfield's own sprites — the stones — were never visible in any
captured frame, so the next question is whether the stage is ending because the
player is being hit by something that is not being drawn, or because the stage
never really ran. Nothing is logged when it ends.

### Why the stage probably ends: exception unwinding is not bound

Nothing is logged when the stage ends, no asset load fails, and the game loop is
demonstrably running (663 `EAGLView` draws in the traced window). Two relocations
are unbound, and together they are the best-supported explanation:

```text
Warning: unhandled external relocation "_OBJC_EHTYPE_$_NSException" in "mroops"
  at 0xde55c, 0xde59c, 0xde5a0, 0xde604
Warning: unhandled non-lazy symbol "___objc_personality_v0" at 0xc87cc
```

`_OBJC_EHTYPE_$_NSException` is the type information a `@catch (NSException *)`
matches against, and `___objc_personality_v0` is the personality routine that
performs the match during unwinding. With neither bound, an app that throws and
catches an exception around its stage setup cannot land in its own handler.
A silent abort back to the menu is what that looks like from outside — which is
exactly the observed behaviour, and it is why nothing appears in the log.

The four `0xde5..` addresses are the catch clauses. Disassembling around them
names the region that throws and settles this in one lookup, the same way
`nl-symbol-at.py` settled the startup fault. Do that before implementing
exception unwinding, which is a large piece of work to start speculatively.

Note the earlier per-frame noise is *not* a fault: the app polls
`[Twitter twIsLogin]` -> `[SA_OAuthTwitterEngine isAuthorized]` and allocates an
`OAToken` every frame, and calls `[[UIDevice currentDevice] userInterfaceIdiom]`
tens of thousands of times. Wasteful, and its own behaviour, not tapHLE's.

### The blank menu bars are drawn text, not a missing image

The two dark bars on the title screen have no visible label. They are supposed
to have one: the app calls
`-[NSString drawInRect:withFont:lineBreakMode:alignment:]` eighteen times during
startup with `[UIFont systemFontOfSize:]`, and logs `setSingleStageButton 50`,
so the labels are composed at runtime rather than shipped as artwork. The small
"MORE GAMES" button beside them, which *is* artwork, renders correctly — so the
contrast is between drawn text and bundled images, not between one button and
another.

tapHLE implements that method (`ui_font::draw_in_rect`, honouring the fill
colour and clipping to the rect) and `systemFontOfSize:` maps to a supported
font, so the call is not a stub. Where the glyphs are lost between there and the
texture the button samples has not been measured. It is cosmetic and does not
block the rating, but it is the cheapest remaining lead into this app's text
path.

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
