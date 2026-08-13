# Changelog

## Unreleased for 0.3.0-alpha.1

- Copy a game's data templates properly instead of ending the game. Games read
  a layout or a set of properties once and then take a private copy for each
  thing they build from it. The call that makes such a copy did not exist, so
  the game quit the moment it was used.

- Read the positions and sizes games store as text without insisting on one
  exact spelling. Games keep the layout of a screen — where each picture and
  button goes — in data files, written as text like `{{0,0},{1024,768}}`.
  tapHLE only accepted the variant with spaces after the commas, and silently
  treated the rest as zero, so every element ended up with no size at all.
  The Jim and Frank Mysteries HD writes its menu one way and its chapters the
  other, which is why its menu appeared and every chapter was a black screen.

- Play the compressed sound files a great many games of this era ship. Apple's
  own conversion tool produced them by default, and tapHLE could not read them
  at all — The Jim and Frank Mysteries HD has 267 sounds and not one of them
  worked, so the game was silent, and its opening story sequence sat waiting
  forever for narration that could never play. Games that wait on a sound
  finishing now get on with it.

- Draw the parts of a game's interface it paints itself, the right way up.
  Panels, buttons and dialogs a game draws by hand — rather than assembling
  from images — were never drawn at all: they were fully working, fully
  touchable, and invisible. Where such a panel covered the screen, as the
  Crystal sign-up screen does in The Jim and Frank Mysteries HD, the result was
  a game that ignored every tap, because the only way past the panel was a
  button nobody could see. They now appear, and appear in the correct
  orientation rather than upside down.

- Let a game get past a pop-up that has only one button. tapHLE cannot draw
  pop-ups, so it tells the game one was dismissed; for a pop-up offering a
  single "OK" it used to say nobody pressed anything, which the game reads as
  the message never having been acknowledged, and it waits there. A pop-up
  with one button is an announcement rather than a question, so that button is
  now reported as pressed. Pop-ups offering a real choice are untouched and
  still press nothing, because picking one would be choosing for the player.
  Mr. Oops!! stopped at its mission briefing and now starts a stage.
- Tell a game its network request failed instead of leaving it waiting.
  tapHLE has no network, and it used to say so by refusing to build the
  request at all — which no real device does even in airplane mode, so no
  game has code for it. Games carried the empty request forward and either
  hung on a sign-in, a leaderboard or an advert that could never answer, or
  quit outright. The request is now built normally and the connection reports
  that there is no internet connection, which is the failure games already
  know how to handle. Mr. Oops!! was quitting during startup on its Twitter
  sign-in, which this gets it through.
- Stop games quitting on startup when they watch the clipboard for changes.
  The names a game needs to ask for those notifications were missing, and a
  missing name is not an error a game can see — it reads as empty and the
  game quits on the spot, nowhere near anything to do with copy and paste.
  Together with the network fix above, this is what gets Mr. Oops!! to its
  title screen.
- Draw the shapes and outlines games ask for directly instead of quitting:
  circles and ovals, rectangle outlines, sets of separate lines such as
  grids and tick marks, and the "cut out the overlap" style of filling.
  Games used these for meters, buttons, minimaps and score panels, and any
  one of them ended the game where it was called.
- Get rectangle arithmetic right where games rely on it: asking whether a
  rectangle is empty, tidying up one that came out backwards, combining two
  into the box that holds both, and slicing a strip off one edge — the
  standard way a screen is divided into a bar and a content area. All four
  used to end the game.
- Remember a drawing colour set as a list of numbers rather than as a colour
  object. This is the older of the two ways to choose a colour and games use
  it constantly; tapHLE could not, because it was discarding the colour
  space that says how to read the list. Saving and restoring drawing state
  also used to lose the outline colour, so a game that changed it
  temporarily kept the change.
- Let games inspect and adjust their own classes while running: finding an
  instance variable by name and reading or writing it, listing a class's
  methods, walking the list of loaded classes, and swapping an object's
  class. Libraries games bundle — JSON parsers, key-value observers,
  analytics — do this routinely, and each call ended the game.
- Tell a game when it has modified a collection while looping over it,
  instead of quitting on the check itself. This is the most widely
  referenced piece of system support tapHLE was missing: every game
  containing a `for...in` loop refers to it.
- Decode the escaped characters in a web address. Games needed this to read
  any value back out of a URL — a score, a name, a setting — and it was the
  single most commonly used method tapHLE did not have. Padding and
  truncating a string to a fixed width, and replacing part of one, are in
  too.
- Stop games quitting when a screen is told the system is running low on
  memory, or when it releases its view. Games override both to free artwork
  and then call through to the system, and it was that call through that had
  nowhere to go.
- Support the 3-D transform type games use for flip and card-turn effects,
  so building one no longer ends the game. Note that a transform handed to a
  layer still does not change what is drawn; this is the arithmetic, not the
  display.
- Report the device's network addresses, and let a game release the list
  afterwards. The release step is in Apple's own sample code for finding
  your own address, so it ran in a great many games and ended every one of
  them.
- Add more standard C that games expect: bounded string searching and
  joining, parsing large numbers, and the older byte-copy spelling.
- Draw gradients. Games of this era used them for almost every background,
  button and title bar that was not an image, and asking for one used to end the
  game on the spot — so a game that drew a gradient anywhere during start-up
  could not run at all. Both the straight-line and the circular kind now draw,
  and the options that say whether the colour continues past each end are
  honoured. The one visible limit: tapHLE still cannot clip, so a gradient meant
  for a small rounded shape spreads sideways across the whole view.
- Say which of a game's own internal checks failed. When a game's built-in
  assertion fires it names the file, line and condition that gave up; tapHLE
  used to throw all of that away and stop with a message about itself instead,
  so a game that had diagnosed its own problem looked like an emulator bug.
  Games that shut themselves down by signalling now report which signal too.
- Stop games quitting on a handful of common system calls they were entitled to
  make: asking an object or method for its name, asking what thread priorities
  are available, setting up a thread's stack, and the bounds-checked string
  formatting that newer compilers emit automatically. Each of these ended the
  game outright, usually during start-up. Chosen by measuring which missing
  pieces stopped the most games rather than whichever turned up next.
- Show lists that set their own section spacing, instead of quitting.
- Let games inspect their own loaded code, which some do at startup to find
  their resources. Previously that ended the game.
- Start landscape games the right way round. A game that only says which way up
  it goes in its code, rather than in its bundle information, used to open
  sideways in a portrait window with its edges cut off, and needed
  `--landscape-left` passed by hand. tapHLE now asks the game at startup.
- Let games create and save their own files. tapHLE told a game that any folder
  it had not made yet already existed, so games skipped creating their save
  directories and then failed, much later, when they tried to write into them.
  Games that ask what went wrong, or that sort saved games by date, no longer
  stop the emulator either. Crafted now generates and saves a world.
- Stop `--headless` crashing during startup on games that ask the system what
  languages the player prefers, which is most of them. The crash reported only
  an internal error, and it happened before the game did anything worth
  watching. Games that need a real window for their graphics still cannot run
  headless, but they now say that instead.
- Show a landscape game's launch image the way it was drawn. tapHLE looked only
  for `Default.png` — the portrait image such a game never actually shows — and
  then turned it on its side to fill the window, so games opened on a squashed,
  sideways splash. Games that ship a left- and a right-handed image, such as
  Baby Monkey, were also handed the one for the opposite orientation and came up
  upside down.
- Stop Unity games aborting when their garbage collector restores access to
  memory it had released. tapHLE has no page protection to apply, so refusing
  the request described tapHLE rather than the game's memory, and Mono shut the
  app down over it.
- Stop claiming OpenGL ES support for vertex array objects, which tapHLE does
  not have. Games that believed the claim drew whole scenes with another
  object's geometry; Cubed Rally Redline's track and terrain were invisible.
- Say which file a failed `open()` was looking for. The warning previously
  printed only the address the filename happened to sit at, so a game quietly
  missing a level or a save gave no clue what it wanted.
- Let older JSON parsers reserve mutable byte storage and access NSObject's
  root ivar offset, advancing Cubed Rally Redline's review and results paths.
- Let OpenGL ES 1 games use non-power-of-two textures and render targets on
  Windows, fixing games that otherwise disable their requested display mode.
- Keep running when a game builds a Foundation collection by hand instead of
  through `+alloc`. JSONKit, which many games bundle to read JSON, does this,
  and it previously ended the app the moment a parsed result came back.
- Stop Unity games dying when their garbage collector recycles memory. Mono's
  collector releases and reclaims its own heap by re-mapping it in place, and
  tapHLE refused, so the game shut itself down — in Cubed Rally Redline, while
  loading a second race.
- Rebrand the fork as tapHLE across the executable, crates, runtime resources,
  internal symbols, tests, and packaging.
- Define Windows game compatibility as the product focus, with macOS retained
  for development and Android outside the active support scope. Running on
  modern iOS remains a likely future direction, but no iOS host ships yet.
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
- Run games built with Unity. The engine now starts, loads its Mono runtime and
  scenes, and reaches gameplay. This needed the 64-bit integer-to-float
  conversion helpers a managed runtime uses for ordinary arithmetic, the UTF-16
  and mutable entry points of `CFString` and `CFData`, detaching a renderbuffer
  from its drawable when a surface is rebuilt, and a Core Motion that describes
  a device without a gyroscope instead of stopping when asked for one.
- Stop freezing when an app asks for memory at a particular address. Where the
  requested address could not be used, only one of the two reasons it might be
  unusable was recovered from, so a mapping could be refused with most of the
  address space free. An allocator that retries — which is what a managed
  runtime does — then never got its memory, and the game sat on its last frame
  looking as though it were still running.
- Cubed Rally Redline reaches its menus, car select and a race, and now draws
  them. It renders landscape directly rather than letting tapHLE rotate the
  screen for it, so it was being rotated twice and the finished frame arrived
  as flat horizontal bands; it now launches in landscape-native mode. Racing,
  crashing, reading the results and starting another race all work.

### Known limitations

- Throwing an Objective-C or C++ exception is still unimplemented and reports
  itself clearly; registering the handlers, which is what almost every app
  actually needed, works.
- UITabBarController and UIToolbar hold and report their contents but are not
  drawn, so an app relying on the tab bar to change tabs stays on the first tab.
- Zoom, pattern colours and content gravity are stored and reported back rather
  than applied.
- Audio queues cannot render offline, which is how Unity decodes compressed
  clips, so those sounds are missing. The call now fails the way a device fails
  it when the codec is busy, which apps are written to survive, rather than
  ending the game.
- Core Motion reports no accelerometer data. Apps reading tilt through
  `UIAccelerometer` are unaffected; those reading it through Core Motion see no
  movement, so tilt steering does not work in them.

Earlier release history belongs to the upstream touchHLE project and can be
found in its repository history.
