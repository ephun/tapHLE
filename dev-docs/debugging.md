# Debugging tapHLE

See also `building.md`.

For selected-game work, begin with `app-debugging-playbook.md` and any existing
`app-notes/<app-slug>.md`. They define artifact identity, the evidence ladder,
bounded instrumentation, the isolated Windows run protocol, and the continuation
handoff.

## Logging

`src/log.rs` provides two logging macros, `log!()` and `log_dbg!()`. The former always prints a log message, whereas the latter only prints a message if the containing module is listed in `ENABLED_MODULES` in the same file.

Some modules you might want to enable:

* The combination of `tapHLE::abi` and `tapHLE::dyld` gives you a trace of almost all guest-to-host calls, among other things
* `tapHLE::mem` logs memory allocations and deallocations

## Debugging crashes in host code

The `RUST_BACKTRACE=1` environment variable is always helpful. You'll probably want a debug (not `--release`) build of tapHLE to get the best output.

## Debugging crashes in guest code

tapHLE will print the basic registers (r0-r13, SP, LR, PC) and a basic stack trace (using frame pointers) for the current thread when a panic occurs. To make sense of the result, you will probably want to open the app binary in Ghidra or another reverse-engineering tool.

## Dumping classes/selectors/function symbols from binaries
The `--dump=linking-info` flag dumps information about the classes, selectors, and lazy symbols (functions) that are requested by the binary, and how tapHLE is handling them. This is output to the file specified by `--dump-file=` (which defaults to the running directory's `DUMP.txt`).

The most useful application for this is determining which classes/selectors/functions that (might) be needed by an application are not implemented by tapHLE. This can be checked with `dev-scripts/log-unimplemented.sh [name of app to check]` (make sure `jq` is installed!).

The schemas for the JSON are described in `ObjC::dump_classes` (in `src/objc/classes.rs`), `ObjC::dump_selectors` (in `src/objc/selectors.rs`), and `Dyld::dump_lazy_symbols` (in `src/dyld.rs`).

### GDB Remote Serial Protocol server

For more complex cases, you can use the `--gdb=` command-line argument to start tapHLE in debugging mode, where it will provide a GDB Remote Serial Protocol server. You can then connect to tapHLE with GDB. (In theory LLDB also should work, but it doesn't.)

A quick word of warning: this will not be the GDB experience you may be used to when writing C/C++ code and compiling it in debug mode. The GDB support was added to help with debugging apps for which we don't have symbols, let alone DWARF info or source code. GDB when connected to tapHLE will not know about local variables or even stack frames! You'll need to know instruction addresses and register numbers. As such, having the binary open in a tool like Ghidra while debugging is practically mandatory.

Anyway, you'll need a version of GDB that supports ARMv6. On Windows, use an
ARM-capable GDB from a cross-toolchain and verify that its architecture list
includes ARM; an x86-only GDB cannot debug the guest. On macOS, the Homebrew
package for `gdb` is multi-architecture. On Ubuntu, `gdb-multiarch` may work.

The basic set of steps is:

* Start tapHLE in debugging mode: `tapHLE --gdb=localhost:9001 'Some App.app'`.
* In a separate terminal window, start GDB: `gdb 'Some App.app/SomeApp'`. (You can omit the executable path, but this leaves GDB with no debug symbol info, [which may be a worse experience](https://sourceware.org/bugzilla/show_bug.cgi?id=30234).) Then, inside GDB, run `target remote localhost:9001` to connect to tapHLE.

If you prefer for GDB to connect immediately: `gdb 'Some App.app/SomeApp' -ex 'target remote localhost:9001'`.

When GDB first connects, CPU execution is paused and none of the guest app's code has been run yet. While execution is paused, tapHLE allows GDB to:

* Read and write registers
* Read and write memory
* Resume execution, either indefinitely or for a single instruction
* Kill the emulated app (this just makes tapHLE crash)

GDB provides various services on top of this, for example:

* `break *0x1000` sets a breakpoint
* `info registers` shows the content of registers
* `backtrace` shows a backtrace (though tapHLE's own may be better)
* `print *(float*)0x2000` evaluates a simple C-like expression
* `layout asm` opens a disassembly view
* `kill` will make tapHLE crash
* `step` resumes execution for a single instruction
* `continue` resumes execution indefinitely

Beware that iPhone OS apps often contain a mix of Thumb functions and normal Arm functions. GDB usually won't know which kind of function it's dealing with:

* When no symbols are available, GDB will assume an address is Arm code by default. You can use `set arm fallback-mode` to change this assumption.
* When full symbols are available, GDB seems to assume symbols are for Arm functions [even when they aren't](https://sourceware.org/bugzilla/show_bug.cgi?id=30386). You can use `set arm force-mode` to override this.

GDB seems to [mostly](https://sourceware.org/bugzilla/show_bug.cgi?id=30385) understand the convention of setting the lower bit of the address to 1 to indicate a Thumb function, and in any case setting an Arm breakpoint in Thumb code (not vice-versa) usually works, so you usually only need to worry about this when disassembling things.

tapHLE only communicates with GDB while execution is paused. Beyond being paused when you initially connect, it is also paused when certain CPU errors occur, or after stepping (resuming execution for a single instruction). Breakpoints are a useful way to force execution to pause at convenient locations. Another option is to press the F12 key while you have the tapHLE window in focus, which will make tapHLE pause during the next NSRunLoop iteration. If the app fails to return to the NSRunLoop then this won't be useful.

## Graphics debugging

[apitrace](https://apitrace.github.io/) is invaluable for figuring out OpenGL-related issues.

More generally, and especially Outside the OpenGL realm, sometimes the most effective solution is dumping image data to a file. There's some functions in [`crate::debug`](../src/debug.rs) that might be useful for this. Don't forget that you can also use Rust's `std::fs::write` if necessary. GIMP and some other tools can read raw pixel data (easiest if the filename ends in `.data`).

### Which way up: two flips, and both of them count

Anything tapHLE draws that has a handedness — text above all — can be turned
over twice on its way to the screen, and it only lands right when both are
accounted for:

1. **The transform.** UIKit lays out y-downward and Core Graphics y-upward, so
   `UIView` installs a y-axis flip around every `-drawRect:` call. Read it as
   the sign of `CGContextGetCTM(...).d`.
2. **The destination.** The compositor draws a `CALayer`'s backing bitmap with
   its vertical texture coordinate inverted, so everything in it is turned over
   once more (`composition.rs`, `rows_are_top_to_bottom`). A bitmap an app
   created itself and uses as a texture or assigns to `contents` is not.

Two flips cancel. So drawing has to be turned back over exactly when the two
**agree**, and left alone when they differ. Nothing about a bitmap's contents
distinguishes the two destinations, so `CALayer` marks the one it owns and
`CGBitmapContextData::flipped_on_presentation` carries the answer.

**The trap is that the transform alone looks like it explains everything.** It
is the visible half, it correlates with the common case, and a fix keyed to it
passes the app in front of you. It was wrong twice:

- Keying on the CTM alone fixed `-drawRect:` text and left every string an app
  drew into its own bitmap mirrored — and newly mirrored strings that had been
  correct, in apps that flip their own context first, which is the right way to
  draw UIKit text into a bitmap.
- Applying the same band flip inside `CGContextShowGlyphsAtPoint` broke text
  that was already correct and fixed nothing, because it moved the same wrong
  question one layer down.

Two rules follow, and they generalise past text:

- **Ask where the pixels are going, not only how they are being transformed.**
  Handedness is a property of the whole path to the screen.
- **Check both destinations before believing a fix.** One app whose text comes
  right is a sample. A layer-drawn label (`UILabel`, a `-drawRect:` view) and
  an app-owned bitmap are different halves of the same question, and a change
  that fixes one can silently invert the other. `dev-docs/app-notes/warlords.md`
  has the worked example, and Tap Tap Revenge 3's menu against OLO's menu is a
  cheap pair to check against.

The same reasoning applies to the *layout* being mirrored rather than the
glyphs: flip about the band the text occupies, never per glyph. A per-glyph
mirror turns the letters the right way up and leaves the lines of a paragraph
stacked upwards, which looks like a font bug and is not one.
