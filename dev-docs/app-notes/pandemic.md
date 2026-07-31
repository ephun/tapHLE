# Pandemic 2.5 compatibility work note

- Branch and last pushed commit: `compat/pandemic` (`c28e7944`, pending push).
- Artifact: local authorized copy, `Pandemic 2.5 (v1.01) [Decrypted].ipa`;
  SHA-256 `0FECC03828D55C77087289CF56456B60C30A26F2652D6D99DB3BB0F47E22ADD5`;
  bundle ID `com.darkrealmstudios.pandemic2p5`, version `1.01`, minimum OS
  `4.2` (from `tapHLE --info`).
- Highest clean committed milestone: the exact release build from `c28e7944`
  keeps a visible, responsive 320x480 window at the main menu (Instructions,
  New Game, Options, Credits) after audio startup.
- Click map: direct visible Windows launch of the verified IPA, wait 15
  seconds for the main menu. A physical mouse click at client `(160, 202)` is
  delivered as a complete touch to `EAGLView`, but does not yet leave the menu.
- Proven: `MainWindow_iPhone.nib` stores `UINibName = View_iPhone` on Engine;
  loading that nib produces the app's `EAGLView`.
- Current implementation: retain `UINibName`/`UINibBundle` in
  `UIViewController`'s keyed-decoder path; add the minimal
  `UIGestureRecognizer`/`UIPinchGestureRecognizer` registration API the
  EAGLView creates during startup; support `sscanf`'s byte-oriented `%c`
  conversion and assignment-suppressed `%*c`, which the app uses after audio
  setup; support `ExtAudioFileSeek` after the app configures its PCM client
  format.
- The app schedules `processFreeBuffer:` through `NSInvocationOperation` with
  scalar callback value `0x1`. Preserve such non-object values while retaining
  real Objective-C callback objects, so the operation reaches its target
  without a null-page access.
- Rejected: menu interaction is not blocked by Windows input delivery; the
  traced click reaches `EAGLView` as `touchesBegan:` and `touchesEnded:`.
- Next discriminator: rerun the exact IPA from the clean commit and identify
  the New Game touch hit-test condition in the app's `EAGLView` path, then
  drive the game into a persistent gameplay loop.
