# Warlords: Call to Arms (iPhone) compatibility work note

This is the **iPhone** build, `greyhoundgames.warlordsapp`. The iPad build,
`greyhoundgames.warlordshdapp`, is a separate app with its own record and note
(`warlords-hd.md`).

- Branch: `compat/warlords`.
- Artifact: `Warlords (v3.086) [Decrypted].ipa` from the maintainer's local
  collection. **Provenance is the local collection, not a verified Archive
  item**, and availability was not re-checked.
- Embedded identity (`tapHLE --info`): display name `Warlords`, bundle
  `greyhoundgames.warlordsapp`, version `3.086`, minimum OS `3.0`, iPhone.

## Highest milestone: 3-star (In game), tapHLE `e9237e15`

A campaign battle is fought and resolved, with no launch options. The route is
`dev-docs/clickmaps/warlords.json`: Play, Campaign, Start Campaign, Continue,
then a marked country, Attack, and dismiss the How To Play overlay. The
battlefield appears with its unit icons and dial, and when it is over the
campaign map returns **with the player's territory expanded and fresh attack
arrows** — which is what makes this a resolved battle rather than a screen that
merely opened.

Nothing app-specific was needed; it ran on `trunk` as it stood.

## Known fault: some text is still mirrored

On the race-select, army and battle screens, part of the text is drawn mirrored
top to bottom — the two bottom buttons on the race screen, the upgrade labels,
the How To Play overlay — while other text on the *same* screen reads
correctly. So this is not the flipped-context bug fixed for `UILabel` and
`NSString` drawing on 2026-08-16; it is a second path that still gets the flip
wrong, and this app is the clearest place to study it because both behaviours
appear side by side in one frame.

### What the mirrored strings actually are

Traced: this screen draws **six** strings through
`-[NSString drawInRect:withFont:lineBreakMode:alignment:]`, and those six are
exactly the mirrored ones. That is the same path the 2026-08-16 fix corrected —
but the fix only acts when the context's y axis is flipped, and here it is not,
so it correctly leaves these alone.

So the two cases differ by **destination, not by the transform**:

- A view's layer bitmap is flipped again by the compositor on its way to a
  texture, so text drawn into it has to be written the other way up. That is the
  case the fix handles.
- A bitmap context the app made itself, and uploads as a texture, is not flipped
  by anything. Text drawn into it must be written the right way up, and is not.

**One attempted fix was wrong and is recorded so it is not tried again**:
applying the same band flip inside `CGContextShowGlyphsAtPoint` broke the text
on this screen that was already correct — the app's own Core Graphics text runs
— while leaving the six mirrored strings mirrored. Reverted.

## Replay on `e7138f01`, 2026-08-17

The recorded route replays to its last step, but **the milestone was not
confirmed on this run** and no report was filed for it. What was seen, from
desktop screen captures rather than the runner's frames:

- The campaign map, the territory panel with Attack and Cancel, and — after
  Attack — the battlefield, with the spear/sword/archer icons, the dial and
  the options gear.
- On the battlefield, a **How To Play overlay** that did not clear. Tapping a
  lane moves the yellow lane arrow, and tapping the spear icon then a lane
  spawned nothing; the overlay stayed up and no battle was fought.

Do not read that as a regression without more evidence. The earlier 3-star run
did not report an overlay, and this run had save state present, so the likeliest
explanation is that it is a first-battle screen the earlier route never met
rather than something that broke. Either way the honest position is that the
recorded milestone is currently unverified.

**Do not trust the runner's frames for this app.** Step 07 captured the
campaign map while the battlefield was actually on screen — `PrintWindow`
returned stale content, exactly as it does for OLO. Replay with `-KeepOpen`
and capture the window off the desktop.

### Next discriminator here

Find what dismisses the How To Play overlay. Until that is known, the route
cannot reach a fought battle, and the map's last two steps describe a screen
this run never got to.

## Next discriminator for the text

Decide the orientation from the destination rather than from the CTM. The
question to answer first is how `CGBitmapContextDrawer` can tell a
compositor-flipped layer bitmap from an app-owned one; if the layer's bitmap
carries that flag, both cases can be served without guessing, and the six
strings here are the check for one side while any `UILabel` is the check for
the other.

The flag belongs on `CGBitmapContextData`, which `CGBitmapContextDrawer`
already carries, set where `ca_layer.rs` creates a layer's backing context.
`composition.rs` is the other half of the evidence: it draws a layer bitmap
with flipped UVs (`rows_are_top_to_bottom = host_obj.contents == nil`) and a
`CGImage` in `contents` unflipped, which is precisely the difference between
the two destinations.

Two more mirrored screens were seen on this run, both consistent with that
account and neither previously recorded: the territory panel on the campaign
map (`Difficulty`, `Owned By:`, the owner's name) and the whole How To Play
paragraph on the battlefield, whose **lines are stacked in reverse order** —
the band-level flip, not a per-glyph one. The headings beside them read
correctly, because those are artwork rather than drawn text.
