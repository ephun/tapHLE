# Changelog

## Unreleased for 0.3.0-alpha.1

- Load a view controller's archived view nib and support registration of pinch
  gesture recognizers, allowing more nib-based games to start their custom
  rendering views.
- Preserve scalar callback values passed through `NSInvocationOperation`, so
  older audio engines can schedule their buffer cleanup safely.
- Rebrand the fork as tapHLE across the executable, crates, runtime resources,
  internal symbols, tests, and packaging.
- Define Windows game compatibility as the product focus, with macOS retained
  for development convenience and Android outside the active support scope.
- Add AI-agent contribution guidance, provenance rules, upstream quarantine
  instructions, game-focused issue forms, and repository policy checks.
- Add a simple self-service guide for people using coding agents to improve a
  game they care about.
- Move the current compatibility record to the live tapHLEdb service while
  retaining legacy JSON records for migration and offline validation.
- Add exact Archive.org compatibility provenance and verified three-star
  in-game milestones for Ricky, Baby Monkey, Cops & Robbers, and SPYmouse HD.
- Add MP3 streaming support, AudioQueue OpenAL source cleanup, clean
  secondary-thread shutdown, and state-aware EAGL frame capture for
  agent-driven Windows testing.
- Start hundreds of apps that previously died before showing anything. Support
  was chosen by surveying where 1501 apps stop and fixing the most common
  causes: Objective-C autorelease-pool and exception-handler entry points,
  runtime class creation, integer-division and C++ allocation builtins, and a
  large set of Foundation, UIKit, Core Animation, Media Player and Audio
  Session constants that apps read during startup.
- Stop treating ordinary runtime conditions as fatal. Retaining a constant
  string, asking for a class that does not exist, an out-of-range array index,
  a mapping the allocator cannot satisfy, and messaging an object that was
  released too many times are all defined or recoverable on a device, and each
  previously ended the app.
- Add UITabBarController, UIToolbar, UIProgressView, UIPageControl,
  NSURLProtocol, NSCondition and NSUbiquitousKeyValueStore, and let nibs decode
  bar items, colours in more colour spaces, and navigation and scroll view
  properties.
- Verified three-star in-game milestones for Tap Tap Revenge 2, Omium,
  Parachute Panic HD and Scoops, and a two-star start for JellyCar.
- Start more apps still: glGetIntegerv now answers for every parameter rather
  than only integer ones, and Objective-C block retention, run-time class
  creation, Core Graphics colour components, Foundation allocation zones and
  extended-attribute calls all work.
- Stop ending the app over ordinary input. A locale identifier carrying a
  script or a numeric region, an OpenGL ES parameter name the backend does not
  know, a nib that produces no objects or sets no view, a message whose
  declared argument types disagree with the receiver's, and an application
  started without a delegate class are all things a device accepts, and each
  one previously aborted at launch.
- Keep going through more ordinary input still. Scheduling a timer that is
  already scheduled, asking for virtual-memory statistics with room to spare,
  reading a localized string from a bundle other than the main one, messaging
  an object whose class is no longer there, loading data that is not valid text
  in the encoding it was declared to be in, loading an image in a format tapHLE
  cannot decode, `dlopen` of a library that is not present, asking a Core
  Foundation array for a starting capacity, enabling or hinting an OpenGL ES
  capability the backend does not model, setting an audio unit property on the
  input bus, transforming by a matrix that has picked up a NaN, and running
  `sscanf` off the end of its input are all things a device takes in its
  stride, and each one previously ended the app where it happened. The same
  reading of the survey also cleared an `Info.plist` that writes its supported
  orientations as a string or a dictionary rather than an array, a table view
  whose delegate sets no row height, and a `CFPreferences` call that names the
  app's own bundle identifier rather than the current-application constant. Of
  the 72 apps a survey of 1501 found stopping at these points, all 72 now get
  past them.
- Read what apps were already asking for. NSScanner scans hexadecimal numbers,
  `NSURL` accepts a `file:` URL wherever a URL string is accepted and reports
  the path of a URL that has a scheme and host in front of it, `.strings` files
  load whether they are UTF-8 or UTF-16 and whether or not their keys are
  quoted, a byte order mark is consumed rather than left at the front of the
  text, and `sscanf` reports the end of its input as `EOF` rather than as no
  match.

### Known limitations

- Throwing an Objective-C or C++ exception is still unimplemented and reports
  itself clearly; registering the handlers, which is what almost every app
  actually needed, works.
- UITabBarController and UIToolbar hold and report their contents but are not
  drawn, so an app relying on the tab bar to change tabs stays on the first tab.
- Zoom, pattern colours and content gravity are stored and reported back rather
  than applied.

Earlier release history belongs to the upstream touchHLE project and can be
found in its repository history.
