# Changelog

## Unreleased for 0.3.0-alpha.1

- tapHLE now has a proper window. Open it, see your apps laid out with their
  icons, click one, and press Play. The app opens in its own window and the
  library stays where it is, so closing a game and starting another does not
  mean starting tapHLE again. Add apps with a button, a menu, or by dropping
  them onto the window. Selecting one shows what it is: who made it, which
  version, which device it was written for, how long you have played it, and
  what the compatibility database says about it.

- Settings you set once apply to everything, and any app can disagree. Each
  app has its own settings page, and anything you do not change there follows
  the general setting — so getting one awkward game to behave does not mean
  configuring the rest of them.

- Several versions of one game are one app in the library. A folder holding
  BabyMonkey 1.01, 1.2.3 and 1.3.5 used to fill three places in the grid and
  push everything else down; now it fills one, and the panel beside the
  library is where you pick which version to play — one place, next to the
  rating and the settings that belong to that build, rather than a dropdown
  repeated under every app. The newest is shown to begin with, the
  choice is remembered, and each version keeps its own settings, rating and
  play time — they are different builds, and only you know which one you
  meant.

- The emulator's output has somewhere to go. A Log panel along the bottom of
  the window carries everything tapHLE prints, with search and filters for
  errors, warnings and the part of the emulator a message came from. It is
  hidden until you ask for it, and it keeps recording while hidden. If an app
  stops unexpectedly, tapHLE now says so and offers you the log, instead of
  its window simply disappearing.

- Compatibility ratings from the tapHLE database are shown beside each app,
  and you can keep your own rating separately without either overwriting the
  other. Reporting a result assembles everything a report needs — including
  whether the app already has a record, so a second one is not created by
  accident — ready to paste into the database. Submitting from inside tapHLE
  is not built yet and says so.

- Every option that could be switched on can now be switched off again:
  `--windowed`, `--portrait`, `--no-landscape-native` and seven more. This
  matters wherever options come from more than one place, because previously
  a setting turned on for a particular game could not be turned back off.

- The emulator's own app picker is gone. It was a second, much smaller library
  screen drawn inside the emulator, and the window described above replaces it.
  Running tapHLE with no app path now says to pass one or to open tapHLE-gui,
  instead of showing the old picker.

- Running tapHLE from a terminal is otherwise unchanged.

- Let games ask which of two moments came first. The question had no answer at
  all, so asking it ended the game.
- Let a game send its own log to a file. Redirecting output that way was not
  supported and ended the game at the attempt.
- Build web and file addresses from text the way games actually write it,
  instead of quitting: any character encoding, and addresses that are already
  complete rather than only bare file paths.
- Show lists whose rows are decorated — with a picture, an accessory, a
  background or an editing control. Setting any of those ended the game, so a
  list that looked slightly fancier than plain text was fatal.
- Let a game say one background job must finish before another. Saying so used
  to end the game.
- Answer a game asking where a shortcut points, instead of quitting. There are
  no shortcuts in the emulated filesystem, and saying so is an answer a game
  knows how to take.

- Answer truthfully when a game asks whether one of its objects supports a
  particular set of features, instead of ending the game over the question. The
  answer is read from what the game itself declared, so it is the same one a
  device would give.
- Stop games quitting when they read settings from a file that is not there.
  Handing on the nothing they got back is ordinary, and it used to be fatal.
- Understand the reply to a web request well enough for games to ask about it —
  its status, its headers, its size. tapHLE has no network, but games name these
  things while handling a failure, and naming one used to end the game.

- Stop games quitting over the parts of their sound setup tapHLE does not
  provide. A game that asks for a mixer, or configures a channel that is not
  there, or leaves a sound format unset because another part of its audio chain
  would have supplied it, used to end on the spot. Those now go quiet rather
  than taking the game with them. Bookworm got its whole start-up sequence back
  this way.

- Stop games quitting when they write an error into their log. Asking an error
  to describe itself is what any logging line does, and it was not answered at
  all, so a game that handled a failure perfectly well died reporting it.
  PapiJump could not get past its start-up network check.
- Let a game ask what one of its interfaces is called. Games use the answer to
  build keys and log lines, and the question had no answer at all, so asking it
  ended the game.

- Text is the right way up. Every piece of writing tapHLE draws for a game —
  button labels, menu items, messages, whole paragraphs — was coming out
  mirrored top to bottom, with the lines of a paragraph stacked in reverse
  order, in any game that draws its interface rather than assembling it from
  pictures. It reads correctly now, wherever the game draws it: on its own
  screens, and on the ones it composes for itself. OLO's menu, Warlords'
  briefing and MazeFinger's in-game labels were all unreadable and are not.

  This also got a game unstuck. Warlords' How To Play screen could not be
  dismissed, because the button that dismisses it was being drawn upside down
  into the dark space below the text, where there was no way to see or press
  it. With the writing the right way up the button is where it looks, and the
  battle behind it starts.

- Shader-based games work and say so. Games needing the newer graphics
  standard drew their frame correctly but it was discarded a moment before
  reaching the screen, so the window stayed black and the game looked broken;
  the picture now appears. tapHLE also stopped announcing at every launch that
  it only supported the older standard, which was untrue and put people off
  trying games that run.

- Let a game ask which screen the player is actually looking at. A game that
  checks this before deciding what to do next used to stop on the spot; the
  answer now accounts for a pop-up screen covering the one underneath, which is
  the case the question is usually asked about.

- Stop games crashing on startup the moment they ask to be told about the
  network. tapHLE remembered *where* the game had put the details of that
  request rather than the details themselves, and by the time it answered, the
  game had reused that memory — so the answer went to the wrong place and the
  game stopped. Flight Control HD could not start at all, and now plays again.

- Tell a game who owns its files and what they may do, instead of nothing. A
  game that asks a file for its owner, its group or its permissions used to get
  no answer at all, and a game that then uses the answer without checking stops
  on the spot — a long way from anything to do with files.

- Let a game get past the sliding menu of choices that comes up from the bottom
  of the screen — the one for "share", "restart", "delete" and the like. tapHLE
  cannot draw it, and it did not exist at all, so a game that offered one ended
  there. It is now reported as dismissed by its Cancel button, the same as the
  pop-ups above, so nothing is ever chosen on the player's behalf.

- Stop ending a game when it compares a piece of text against a setting it does
  not have. Looking something up, finding nothing, and comparing anyway is
  harmless — the missing one simply sorts first — and it used to end the game.

- Read a text file without being told what encoding it is in, and say which one
  it turned out to be. This is the modern way to load a text file a game did not
  write itself, and it did not exist, so the game stopped there.

- Support the plain "collection of unique things" games keep in the older,
  lower-level style — creating one, adding to it, counting it, emptying it and
  walking it. None of it existed, so a game that kept, say, the set of notes
  currently on screen ended the moment it made one. Tap Tap Revenge 3 does this
  as a song starts loading.

- Show the contents of a list a game builds once and never refreshes. Lists
  filled in as a screen is prepared came up empty, because tapHLE only ever
  asked a game for its rows when the game explicitly demanded a refresh —
  something a game has no reason to do the first time. Tap Tap Revenge 3's song
  list was blank and now shows its three songs with their artwork.

- Stop ending a game when it prepares a call and leaves some of its arguments
  empty. Leaving them out is ordinary — the missing ones simply count as
  nothing — but tapHLE treated it as a mistake and quit. Tap Tap Revenge 3 did
  this while opening its main menu and never got there.

- Let games slide one screen in over another. The names for which edge such a
  slide comes from were missing, and a missing name reads as nothing at all, so
  a game asking for one stopped dead — nowhere near anything to do with
  animation. Tap Tap Revenge 3 did this the moment its song list opened.

- Let a game set up an object from a whole block of saved settings at once,
  rather than one setting at a time. Games that keep the description of a
  scene in a data file hand the whole description over in one go, and that
  way of doing it did not exist, so the game ended the moment a scene was
  built. The Jim and Frank Mysteries HD builds every element of every scene
  this way.

- Fill in the settings of a scrolling view. A game configures such a view
  before showing it — whether the scroll bars appear, where the content
  starts, how far it is inset — and then asks it what state it is in. Six of
  those settings were missing and any one of them ended the game. The letter
  that opens The Jim and Frank Mysteries HD's first chapter is one of these
  views.

- Stop insisting a rectangle's centre survive being trimmed to the last
  decimal place. Trimming a margin off a rectangle is one of the commonest
  things a game does when laying out a screen, and tapHLE checked its own
  arithmetic afterwards with a comparison that ordinary numbers fail, so the
  game ended. Trimming away more than a rectangle holds is now answered the
  way it should be, too, instead of ending the game.

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
- Look up a value through a chain of properties rather than only one step. A
  game asking for something two or three objects away — the name of the player
  of the current game — was asking a question tapHLE had no answer for at all,
  and the attempt ended it.
- Tell a game how the pixels of a picture are arranged. A game that asks
  before reading a picture's pixels itself — which is what a game does to
  build its own textures — was asking something tapHLE could not answer at
  all, and the attempt ended it during start-up. Fieldrunners for iPad stopped
  there.
- Stop games quitting while setting up a web view. Saying which kinds of text
  in a page should become tappable — phone numbers, addresses, links — is
  something a game does right after making the view, and it was not understood
  at all, so the game ended before the screen holding the view was ever built.
  tapHLE still draws the view blank, as it always has.
- Let a game ask a collection for the copy of a value it already holds, rather
  than only whether it holds one. Games use this to keep a single shared copy
  of a name or a setting, and the question could not be asked at all, so
  asking it ended the game. PapiJump stopped there during start-up.
- Measure a line of text the way a game asks for it when the line has a
  width to fit into. tapHLE could measure text on its own and text inside a
  box, but not this third way, which is what a game uses to lay out a label
  before drawing it — so asking ended the game. Ragdoll Blaster 2 and its
  free version both stopped there.
- Stop games ending when they look something up under a key that turned out
  to be nothing. Finding nothing is an ordinary answer, and games are
  written to take it, but tapHLE stopped on the spot instead. Every version
  of Fieldrunners died this way while building its first screen.
- Divide and take a remainder in one step, the way some games' arithmetic is
  built. tapHLE understood one of the two spellings compilers use for this
  and not the other, and a game built with the other could not do that
  arithmetic at all, so it never started. OLO 1.31 stopped there.
- Stop games quitting while arranging a layer's contents. Saying how a
  picture should sit inside the space it is given is something a game does
  while building its screen, and the instruction was not understood at all,
  so the game ended there. Zippo Lighter 1.1 stopped during start-up.
  tapHLE still stretches contents to fit, as it always has.
- Stop games quitting when they ask an object for a setting it does not
  have. Asking for something that may not be there is how optional
  configuration is read, and a game handles being told there is nothing —
  but tapHLE ended the game over the question. All four versions of
  Spaceteam stopped during start-up. Setting an absent value was already
  survivable; now reading one is too.
- Sort text the way a game asks for it, ignoring case. The comparison did
  not exist at all, so a game sorting a list of names ended there — every
  version of Doodle Jump stopped during start-up. Sorting text with
  accented or non-English characters also no longer ends the game: the
  order is not yet the one a device would give, but that is a better
  answer than stopping.
- Let a game respond when the player changes page with the row of dots at
  the bottom of a screen. tapHLE treated that row as decoration rather than
  as a control, so a game asking to be told about it ended there. Both
  Carnivores games stopped during start-up.
- Give games the evenly-spread random numbers they ask for. The call had no
  implementation at all, so a game using it ended at the first one it
  needed — all four versions of Spaceteam stopped during start-up.
- tapHLE can play back a recorded route through a game by itself, with
  `--replay`. It presses the buttons internally rather than moving your
  mouse, so it works the same on every system tapHLE runs on, it does not
  need its window in front, and it cannot wander off and click something
  else on your desktop while it runs.
- Stop games quitting when they turn their text into bytes. tapHLE knew a few
  ways of writing text out and ended the game on any other, and on any letter
  outside plain English even in the ways it did know — so a name with an accent
  in it was fatal. Three versions of Super Hexagon and Don't Look Back all
  stopped during start-up on this, and now get as far as asking for the network
  instead. A conversion tapHLE cannot make is simply reported as not made, which
  is the answer games are written to expect, and the ones it can make it now
  makes exactly, including the two-byte way of writing text used worldwide.
- Answer a game that asks how this device reaches the internet. Games ask on
  start-up, usually through an advertising or analytics kit, and the question
  had no answer at all: three versions of Super Hexagon, Don't Look Back and
  two of Fieldrunners all stopped there before showing anything. The answer is
  the true one for a machine with no proxy set — nothing in the way, connect
  directly — and it is a real answer rather than a blank, which matters because
  a game handed a blank reads it as an address and stops on the spot.
- Save a game whose saved numbers are large or unsigned. Two kinds of number
  could be held but not written, and a game that put one in its save file was
  ended by the attempt to save — all four versions of Doodle Jump stopped this
  way during start-up, before drawing anything.
- Let a game read the end of a file. Asking for more bytes than are left, or
  reading an empty file at all, ended the game instead of giving it what was
  there — which for a game reading its own settings on start-up is fatal the
  first time it runs, before any settings exist. Mr.Oops!! stopped five seconds
  in, reading four bytes of an empty save, and now runs on.
- Tell a game what kind of thing it is holding. A game handed a value by the
  system — out of a settings list, a saved file, or an answer from the phone —
  asks whether it is text, a number, a yes-or-no, a list or a table before
  using it, and the question could not be asked at all. Three versions of
  Super Hexagon and Don't Look Back stopped there during start-up. Comparing
  two such values now works for every kind as well, where before anything but
  text ended the game.
- Let a game ask its own graphics program what settings it has. Modern-style
  games look through the list when they start, to connect each setting to the
  value they will feed it, and the question did not exist, so the game ended
  at the first one. Organ Trail Director's Cut now starts and keeps running;
  Ski Safari and Temple Run 2 get past it and stop later on other things.
- Set up a scrolling view the way games actually set one up. Six of the
  switches a game flips while building one did not exist, and flipping any of
  them ended the game — one at a time, so fixing one only revealed the next.
  Doodle Jump and Doodle Jump HD died during start-up on three of them in a
  row and now reach their main menu, drawn correctly.
- Let a game save into a container it prepared itself. There are two ways to
  write a saved game and only one existed, so a game that makes its own space
  for the save and hands it over — which is what a game does when it wants to
  add something of its own alongside — ended at the attempt. Doodle Jump
  v3.1.1 and v3.4 both save this way.
- Save a file whose contents a game is still building. Three kinds of value
  were only recognised in their finished form, so a game that saved one it
  could still change — the ordinary case when the game assembled the save
  itself — ended at the attempt rather than saving.
- Let a game say it has finished reading a saved file. Saying so was not
  possible, so a game that loaded its save successfully was ended by the line
  that closes the file. Doodle Jump v3.1.1 and v3.4 both stopped there.
- Survive a game asking for tilt readings at an impossible rate. Two games ask
  for them infinitely far apart during start-up — the result of dividing by a
  frame rate that is zero — and tapHLE tried to honour it and ended the game on
  the first reading. Doodle Jump v3.1.1 and Doodle Jump HD both now start and
  draw their menus.
- Support the kind of animation that moves something through a series of
  positions rather than straight from one to another. The whole kind did not
  exist, so a game that used one ended the moment it made it. Doodle Jump
  v2.7.1 and v3.4 both do, during start-up.

### Known limitations

- Throwing an Objective-C or C++ exception is still unimplemented and reports
  itself clearly; registering the handlers, which is what almost every app
  actually needed, works.
- UITabBarController and UIToolbar hold and report their contents but are not
  drawn, so an app relying on the tab bar to change tabs stays on the first tab.
- Zoom, pattern colours and content gravity are stored and reported back rather
  than applied.
- An animation through a series of positions travels straight from the first to
  the last instead of visiting the ones in between, so it ends where the game
  intended but does not follow the shape the game drew.
- Audio queues cannot render offline, which is how Unity decodes compressed
  clips, so those sounds are missing. The call now fails the way a device fails
  it when the codec is busy, which apps are written to survive, rather than
  ending the game.
- Core Motion reports no accelerometer data. Apps reading tilt through
  `UIAccelerometer` are unaffected; those reading it through Core Motion see no
  movement, so tilt steering does not work in them.

Earlier release history belongs to the upstream touchHLE project and can be
found in its repository history.
