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
  ★☆☆☆☆); report 67 (2026-08-12, tapHLE `e0645ce4`, ★★☆☆☆); report 68
  (2026-08-12, tapHLE `6d54d36d`, ★★★☆☆).

## 2026-08-12: three stars. It plays.

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

### Frontier: the stage plays, and it persists

Pressing the upper menu bar starts the game. The click map, in client
coordinates of the 480x320 window:

1. Launch, wait about 15 seconds for the title screen.
2. Click `(375, 179)` — the upper of the two dark bars on the right ("Begin
   Game"). It highlights purple.
3. The app raises a one-button alert, `【First Mission】Clear 20waves to unlock
   "Iron Cannons"!`. On `6d54d36d` that alert reports its only button as
   pressed, so the app continues.

Stage 1, "Rolling Stones", then runs a real loop: the player is drawn on the
grid, stones spawn with entry arrows and roll across it, and positions advance
between frames. **The maintainer play-tested it directly and confirmed the loop
persists across sustained play.** Left alone it lasts about five seconds and
returns to the title, because the player is hit — that is the game working, not
a defect.

Reported as tapHLEdb report 68 (2026-08-12, tapHLE `6d54d36d`, ★★★☆☆).

**Correction, recorded because I got it wrong and the wrong version was briefly
in this note.** I first read the five-second return as "the loop does not
persist" and looked for a defect. That came from sampling frames at 500 ms and
classifying them by mean brightness: the playfield and the stage-intro banner
have almost the same mean, so a run that was actually playing looked like a
banner that never advanced. Capturing at 250 ms and *looking at the frames*
showed stones moving. On this app, look at the picture before trusting a
summary statistic — and the maintainer watching the window is faster than either.

### Rejected: exception unwinding is not the problem

The wrong reading above led to a wrong cause, which is also recorded so nobody
implements it. `_OBJC_EHTYPE_$_NSException` and `___objc_personality_v0` are
genuinely unbound, and that looked like a `@catch` that could never match.

It is not what happens. tapHLE's `objc_exception_throw` logs
`Ignoring Objective-C exception` and returns, and **that line never appears in
this app's log**, so nothing is ever thrown. The unbound relocations sit in
exception tables that are only read while unwinding, and no unwinding occurs.
Do not implement SjLj unwinding for this app.

### The blank menu bars: measured, and not tapHLE's text renderer

The two dark bars have no label. They are supposed to read "Begin Game" and
"Options" — those exact strings are drawn with
`-[NSString drawInRect:withFont:lineBreakMode:alignment:]` during startup.

Measured with a temporary diagnostic in `ui_font::draw_in_rect` and
`ui_font::size_with_font` (removed again; do not look for it):

```text
draw_in_rect  text="Begin Game" rect=11.0x1.0  fill=(1,1,1,1) ctx=16x1
size_with_font text="Begin Game" font_size=36.0 constrained=Some(11.0x1.0)
```

So the font is right (36 pt, a supported face), the fill colour is right
(opaque white), and the drawing code is not a stub. **The app asks for a 16x1
pixel image context and tells UIKit to draw into an 11x1 rect.** Nothing can be
legible at that size, and `size_with_font`'s absurd 415-pixel height is a
consequence of wrapping "Begin Game" into an 11-pixel width, not a cause.

Where the 11x1 comes from is the open question. The app never calls
`sizeWithFont:` at all — every `size_with_font` call in the run originates
inside `draw_in_rect` — so it is not being misinformed by a measurement API. Its
own numbers fit `charCount * fontSize / 32` closely across nine labels at two
font sizes, which looks like a texture size computed in 32-pixel tiles and then
used as a pixel count.

The likely shape is a fallback: the game's normal path is its own bitmap-font
artwork, and `void load(NSString *kind) failed!` shows asset loads failing at
startup. Check what those loads want before touching the text renderer.
Cosmetic; it does not affect the rating.


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
