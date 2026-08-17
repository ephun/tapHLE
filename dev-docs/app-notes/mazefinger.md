# MazeFinger+ compatibility work note

- Branch: `compat/mazefinger`.
- Artifact: `MazeFinger+ (v1.5.1) [Decrypted].ipa` from the maintainer's local
  collection. **Provenance is the local collection, not a verified Archive
  item**, and availability was not re-checked.
- Embedded identity (`tapHLE --info`): display name `MazeFinger+`, bundle
  `com.ngmoco.MazeFinger`, version `1.5.1`, minimum OS `2.2.1`, iPhone.

## Highest milestone: 3-star (In game), tapHLE `a5127ff1`

Level 1-1 plays. The route is `dev-docs/clickmaps/mazefinger.json`: OK, Play,
touch to go, then drag up the corridor — the player's glow follows the finger
with its lightning trail, which is the step that proves control rather than
mere animation.

## What it took

Three general fixes, none specific to this app, each of which was a hard stop:

1. `-[NSObject conformsToProtocol:]`, answered from the class's own adopted
   protocol list in the binary rather than guessed.
2. `+[NSPropertyListSerialization propertyListFromData:...]` returning nil for
   nil data instead of panicking in the borrow. The app reads a settings file
   that is not there and hands the nothing it got straight on.
3. `NSURLResponse` and `NSHTTPURLResponse`, which did not exist. The app names
   the class while handling its promo-playlist request, which cannot succeed
   here — tapHLE has no network — and naming a missing class ended it.

The third is the widest of them: twenty-six apps in the collection reference
`NSHTTPURLResponse`.

## Known fault at this rating

In-game text is drawn mirrored — the level number and the "Touch to go" prompt
— while the menu text before it is correct. Same second text path as
`warlords.md`, which has the sharper account of what distinguishes the two.
