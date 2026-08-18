# Warlords HD compatibility work note

- Branch and starting commit: `compat/warlords-hd` from `3c6fc0bd`.
- Canonical artifact: `https://archive.org/details/ios-ipa-greyhoundgames.warlordshdapp`,
  `Warlords HD:Call To Arms (v3.065) [Decrypted].ipa`; MD5
  `c9275f6f440a8fad6bd72b4fc8905fc8`, SHA-1
  `3f74c1721a9ba98e4794f06062dca4111973b46c`, SHA-256
  `13646ccf0b14cf7c9c697a64eb132e80e481cd4568969ce4667bdddcd3692e73`.
  The Windows-safe local filename omits the canonical `:` and is explicitly
  mapped to this Archive filename.
- Embedded identity verified with `tapHLE --info`: display name `WarlordsHD`,
  bundle `greyhoundgames.warlordshdapp`, version `3.065`, minimum OS `3.2`,
  iPad device family. `Info.plist` declares `UISupportedInterfaceOrientations`
  = LandscapeLeft, LandscapeRight only; status bar hidden.
- Availability check (2026-07-25): Apple's US bundle-ID lookup returned no
  current listing for this exact build. This is a project-scope availability
  fact, not a legal conclusion.

## Highest milestone (2026-07-25): 3-star gameplay loop

Driven by real foreground input, the app runs its full campaign loop into a
live real-time battle: main menu -> Play -> Campaign -> "Choose Your Race" ->
"Upgrade Army" -> the strategic (Risk-style) campaign map -> tap a territory ->
attack-confirmation dialog -> the battle screen, where selecting a unit and
touching a lane spawns it into combat. The battle renders and persists. This
is a 3-star result (gameplay loop starts and persists).

Two general fixes were needed past the 2-star menu (both graduate to trunk):
`CGBitmapContextCreate` zero-`bytes_per_row` handling, and
`+[NSPropertyListSerialization dataFromPropertyList:format:errorDescription:]`
accepting a non-null error out-parameter.

The route is recorded as `dev-docs/clickmaps/warlords-hd.json` and was replayed end to end
on `e4e0d528` on 2026-08-17. Replay the map rather than retyping the steps
below; the prose is kept for the reasoning behind the coordinates, and the map
is what should be kept correct.

Proven input recipe (client coords, 1024x768 landscape-native window; ~15 s
menu wait, then real foreground mouse taps): Play (95,418); Campaign (95,415);
Start Campaign (870,745); Continue (910,745); territory "Vorth" (700,370);
Attack (88,358); Play Now (380,602) to dismiss the tutorial; select spear unit
(58,75); spawn on a lane (450,400).

The 2-star menu milestone below still holds and is reached by this earlier
chain of general emulator additions (each was the exact next hard blocker):

1. `objc_msgSendSuper2_stret` — struct-return super2 dispatch (the WIP that
   started this branch).
2. `-[NSString rangeOfCharacterFromSet:options:range:]`.
3. `CFDictionaryGetValueIfPresent`.
4. `+[NSCharacterSet alphanumericCharacterSet]`.
5. `-[UIFont fontName]` / `familyName` / `pointSize` (UIFont now stores its
   name).
6. `CGFontCreateWithFontName` (substitutes a bundled Liberation face by name).

These are all reusable and graduate to `trunk`.

## Proven facts

- Rendering path is EAGL (OpenGL), fullscreen fast path. Warlords is a
  **landscape-native renderer**: with the portrait-shaped screen it drew a
  1024×768 scene into a 768×1024 renderbuffer (right edge clipped, ~256px band
  on the long axis) and tapHLE then rotated it, so the menu came out sideways
  and cut off.
- With `--landscape-native` the emulated screen is landscape (1024×768), so the
  fullscreen EAGL layer/renderbuffer is 1024×768, the app draws into it 1:1, and
  it is presented with identity rotation. Captured renderbuffer is now 1024×768
  and the full menu renders correctly (title, both unit sprites, all four menu
  items, corner icon).
- Contrast with a portrait+rotation landscape game (Ricky): it presents a
  480×320 Core Animation frame via composition and is unchanged by this option.

## Rejected hypotheses

- "A different `--landscape-*` flag fixes it." No — the clip was a
  renderbuffer-size mismatch (768 wide vs the app's 1024-wide viewport); it took
  a landscape-shaped screen (`--landscape-native`), not a presentation-side flag.
- "`--force-composition` fixes it." No — it changes the present path but not the
  portrait drawable the app renders into, so the clip persists.

## Next discriminator

3 stars is reached. Beyond it is human-tested territory (4+): play a battle to
a win/loss, confirm the strategic map updates and the campaign advances across
multiple territories, and check audio and sustained input. An agent must not
assign 4-5 stars. Watch for later blockers during a full battle (more units,
projectiles, win/lose screens) and any audio-path TODOs.

## Checks run (this session)

- `cargo fmt --all -- --check`: clean.
- `cargo test --workspace --lib` (debug and release): pass (86 + 4 tests).
- Live run: hash-verified IPA, release build, `--landscape-left`; frame capture
  of the main menu.
