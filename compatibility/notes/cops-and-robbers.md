# Cops & Robbers compatibility work note

Last updated: 2026-07-23. Rating: ★★★☆☆ (3/5) In game.

## Identity and source

- Work branch: `compat/cops-and-robbers`.
- Canonical Archive item:
  <https://archive.org/download/ios-ipa-collection/Cops%20&%20Robbers%201.5.ipa>
  (identifier `ios-ipa-collection`).
- Archive filename: `Cops & Robbers 1.5.ipa`, `source: original`.
- Archive size: 6,060,474 bytes.
- Archive MD5: `66b9fc24b834627f0dfe5db79f28c07a`.
- Archive SHA-1: `da6e0e7d97af4a704be6a7dd63c8b5bbf10eeeaf`.
- Locally calculated SHA-256:
  `73d30a59a8d636e7c1cc33fd29e1a720a1f10badae40570c2f12aa5649e538a9`.
- Embedded bundle: `com.glu.thief3d`, display name `Cop&Robber`, version `1.3`,
  minimum OS `2.0`, iPhone device family.
- Build date: the archive entries for the executable and `Info.plist` are both
  stamped `2009-06-16`, and the earliest asset is `2009-04-12`. That is the
  evidence behind the `release_year` of 2009 in the database record; no iOS
  `Info.plist` carries a release year, so this is the only source in the
  artifact itself.

Live metadata was verified on 2026-07-23 and the downloaded bytes match its
exact size, MD5 and SHA-1. As with Baby Monkey, the Archive filename and the
embedded version disagree: the file is named `1.5`, `tapHLE --info` reports
`1.3`. Keep the Archive filename as the canonical source name and treat the
embedded version as authoritative; do not claim these bytes are 1.5.

Availability was checked on 2026-07-23. Apple's Lookup API returned no result
for `com.glu.thief3d` in the US, GB, CA, AU, DE, FR or JP storefronts, and a
name search surfaced no current Glu title matching it. This is a bounded
availability observation, not proof of legal abandonment.

This is an early app: it loads the **armv6** slice (not armv7), it is
**nib-based** UIKit rather than a hand-rolled engine, and it renders its 3D
through EAGL at 320x480 portrait.

## Current checkpoint: gameplay runs

Reproduced from the clean committed release build of `c4d56964` (window title
`Cop&Robber (tapHLE c4d56964)`, no `-dirty`) against the hash-verified bytes, in
a normal visible window from an isolated `%TEMP%` run directory.

The whole front-of-game path works: language select, the sound-on question, the
title card, a multi-scene 3D intro cutscene with dialogue, the main menu (PLAY /
EXTRAS / HIDEOUT / OPTIONS / HELP), the 3D character customisation screen, the
tilt-controls tutorial, and then gameplay.

In gameplay the character runs down a 3D city street with obstacles,
collectibles, a countdown timer, a score, a PAUSE button and a skyline HUD. Over
one 18-second observation the timer ran 2:48.1 -> 2:31.3 and the score rose
300 -> 800, so the clock, the collection logic and the scene are all live. The
process stayed alive and animating throughout.

### Proven input recipe

The route is recorded as `compatibility/clickmaps/cops-and-robbers.json` and was replayed end to end
on `e4e0d528` on 2026-08-17. Replay the map rather than retyping the steps
below; the prose is kept for the reasoning behind the coordinates, and the map
is what should be kept correct.


Client coordinates as fractions of a 320x480 client area, with a title-bar focus
click first (see the playbook). Each tap is a real foreground mouse event.

1. `0.13,0.93` SELECT — accept the default language (English).
2. `0.13,0.93` SELECT — accept the default answer to the sound question.
3. `0.88,0.94` x4, a few seconds apart — SKIP through the intro cutscene.
4. `0.13,0.93` SELECT — PLAY, which opens character customisation.
5. `0.13,0.93` DONE — returns to the main menu.
6. `0.13,0.93` SELECT — PLAY again, which opens the tilt-controls tutorial.
7. `0.50,0.55` — tap the play area to dismiss the tutorial and start the run.

Do **not** tap SKIP on the tutorial screen: there it means "back" and returns to
the main menu. That cost several runs to notice.

Taps are confirmed to reach the game, not merely to coincide with timed
transitions: tapping `Español` on the language screen switched every later
screen to Spanish (`OMITIR` for SKIP, Spanish dialogue).

## Fixes this required

All three are general, none is specific to this game.

1. `55af471f` — `UIView`'s four co-ordinate conversion methods resolved a nil
   counterpart by asserting the receiver had a window. A nib-based app converts
   while laying out, before anything is mounted, so this aborted at launch.
   `CALayer`'s conversion already resolves a nil layer to the top of the
   hierarchy, so the nil is now passed straight down.
2. `582ace5f` — `AudioServicesAddSystemSoundCompletion` did not exist. Added
   with its Remove counterpart, and the routine actually runs: the run loop
   polls each sound's OpenAL source and fires on the playing-to-stopped edge.
3. `c4d56964` — `EAGLGetVersion` was not exported. Reports 1.0.

## The loop closes, and steering works (2026-07-23, later)

Two further things were checked on the same committed build, because "the loop
starts and persists" deserves more than one run's worth of evidence.

**Steering responds to the accelerometer.** tapHLE simulates tilt with the right
mouse button held while the cursor moves. Holding it and dragging left moved the
character's on-screen x from about 145 to about 85; dragging right afterwards
carried him back rightward to about 122. The displacement followed the input in
both directions, so the accelerometer path reaches the game. Shortly after the
right drag the round ended, which is consistent with steering into the dumpster
visible in those frames -- but the death frame itself was not captured, so treat
the cause as unconfirmed.

**A round ends and another can be started.** Left alone, a round finishes and
the game replaces the HUD with a "TAP THE SCREEN TO START" prompt over the
idling character. Tapping that prompt starts a fresh round: timer 1:59.4, score
800, character running, obstacles ahead, PAUSE present. So the cycle
play -> end -> restart completes under tapHLE.

No score or game-over screen was seen between the round ending and the start
prompt; the game appears to go straight there.

## Audio produces signal (2026-07-23, later)

Measured rather than assumed, with the OpenAL Soft wave writer described in the
playbook. A 111.9 s capture (stereo, 44.1 kHz) driven through the proven recipe
into gameplay:

- Silent for the first 21.5 s, and again from 23.9 s to 34.9 s. That covers
  startup and the language/sound prompts.
- From about 34.9 s onward there is no silence of 3 s or more: the intro, the
  menus and gameplay all carry signal.
- Menu and intro window: mean −36.1 dB, max −14.5 dB. Gameplay window: mean
  −32.1 dB, max −11.5 dB, so gameplay is the louder part.

This proves the host mixer emits signal. Per the playbook it does not prove
default-device routing or that the audio sounds right, so treat the feature as
partial until a person listens.

## Known limitations at this rating

- Jump and the speed boost were never exercised; only lateral steering was.
- Only the first mode was played, for two rounds. Whether a round can be
  *completed* rather than lost is unknown, as are PAUSE, HIDEOUT, EXTRAS and
  OPTIONS.
- A file the app repeatedly `open`s (`O_RDONLY`) and `remove`s does not exist,
  producing a steady trickle of warnings. It does not appear to matter. It is
  *not* worth another static hunt: the path argument is a guest heap address, so
  it is composed at runtime, and a scan of all 16,561 printable strings in the
  armv6 executable turns up no candidate filename. Identifying it needs a
  diagnostic build that logs the resolved path in `libc::posix_io`.
- `UIProxyObject` for `IBFirstResponder` is left unreplaced at nib load. It has
  not caused an observed problem, but it is a real gap.

## Next discriminator

Steering and the play/end/restart cycle are now confirmed, so the open boundary
is whether a round can be *won* rather than lost: play deliberately, avoiding
obstacles, and see whether the timer can be run down to zero and what the game
does then. After that, PAUSE and the HIDEOUT/EXTRAS/OPTIONS screens. Audio is
measured (see above) but has not been listened to, so a human ear on it would
move that feature from partial to confirmed.
