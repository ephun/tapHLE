# Percy compatibility work note

Last updated: 2026-07-18.

## Identity and source

- Work branch: `compat/percy`.
- Archive item: <https://archive.org/details/ios-ipa-com.deluxe.pipes>.
- Tested file: `Percy Jackson & The Olympians: The Lightning Thief (v1.0) [Decrypted].ipa`.
- SHA-256: `67a4981113d76f796fd706be321db92d2ca376e3ec83c333478cc8fc63f680ca`.
- MD5: `84a94d85e26af00b4594d7bba03cae77`.
- SHA-1: `d6860631a08b4909dd93dc11b50fb22a49ac2809`.
- Bundle: `com.deluxe.Pipes`, version `1.0`, minimum OS `3.1.2`.

The verified IPA is also kept locally in `tapHLE_apps` for playtesting. All
IPA/app binaries and extracted assets for every compatibility target follow
the same rule: keep them local, ignored by Git, and do not redistribute them.

## Current checkpoint

The current Percy work reaches live gameplay on Windows. The
navigation-controller NIB stack is restored, modal presentation and dismissal
work through the level intro, C++ object constructors/destructors run in the
required order, and the game creates drawable storage. Same-process captures
before and after a controlled tap at `(148,172)` show the selected tile changing
from a vertical segment to a horizontal segment while the randomized board
stays fixed.

The Instructions screen now decodes and renders its archived text, font, color,
and bullet spacing. Its **Main Menu** button pops the view controller without a
crash and returns to a framebuffer identical to the known-good menu.

The CAF/AIFF audio paths now decode Percy's PCM effects, including the CAF
asset whose data chunk has a nonzero edit-count field. Sound was confirmed in
manual playtesting, although longer sessions and every effect still need
broader validation.

## Known gaps

- Full-session stability, saving, and every game mode are not yet validated;
  the custom-level crash belongs to the separate Ricky investigation, not
  Percy.
- Input coverage is only partially validated; continue with deterministic
  taps and then expand to gestures and controls.
- The desktop screenshot path can appear black with OpenGL; use the EAGL or
  presented Core Animation capture that matches the active rendering path, then
  confirm behavior manually.
- General dirty-layout scheduling and untested game APIs remain future work.

## Reproduction route

The route is recorded as `compatibility/clickmaps/percy.json` and was replayed end to end
on `e4e0d528` on 2026-08-17. Replay the map rather than retyping the steps
below; the prose is kept for the reasoning behind the coordinates, and the map
is what should be kept correct.


From the repository root, run the release executable with no path to use the
picker, or pass the exact verified IPA path directly. For the current route:

1. Pick Percy and tap **Play** on the main menu (near `(35,300)`).
2. Select **Level 1** (client coordinate near `(58,135)`).
3. Tap **Play** on the level-intro screen (near `(420,120)`).
4. Wait for the board, then tap a pipe tile (the deterministic harness uses
   portrait client coordinates near `(148,172)`).

The main-menu **Instructions** button is near `(150,299)`. On that screen,
**Main Menu** is near `(240,289)`.

Use a title-bar click first when automating Windows input so the first client
click is not consumed solely by focus activation. Record the exact Archive
filename and hashes in every compatibility report.
