# Pandemic 2.5 compatibility work note

- Branch and last pushed commit: `compat/pandemic` (pending first checkpoint).
- Artifact: local authorized copy, `Pandemic 2.5 (v1.01) [Decrypted].ipa`;
  SHA-256 `0FECC03828D55C77087289CF56456B60C30A26F2652D6D99DB3BB0F47E22ADD5`;
  bundle ID `com.darkrealmstudios.pandemic2p5`, version `1.01`, minimum OS
  `4.2` (from `tapHLE --info`).
- Highest baseline: creates a window, then aborted while a nib-decoded Engine
  controller was given a fallback `UIView` rather than its archived `EAGLView`.
- Proven: `MainWindow_iPhone.nib` stores `UINibName = View_iPhone` on Engine;
  loading that nib produces the app's `EAGLView`.
- Current implementation: retain `UINibName`/`UINibBundle` in
  `UIViewController`'s keyed-decoder path; add the minimal
  `UIGestureRecognizer`/`UIPinchGestureRecognizer` registration API the
  EAGLView creates during startup; support `sscanf`'s byte-oriented `%c`
  conversion and assignment-suppressed `%*c`, which the app uses after audio
  setup.
- Next discriminator: rerun the exact IPA from the clean commit and identify
  the next post-`sscanf` startup boundary.
