# Help a game work in tapHLE

You can use your own coding agent to improve a game. You do not need to know
how to program.

No one else is required to work on your request. You and your agent lead the
work. Agents can make mistakes. You stay in control, and you should read the
agent's summary before you publish anything.

## Before you start

You need:

1. A supported test environment: a Windows computer.
2. A fork or clone of tapHLE.
3. A coding agent that can work in that folder.
4. Lawful access to the exact game version you want to test.

Never upload an IPA or game files to GitHub. Keep them outside the repository.
The `tapHLE_apps` folder is ignored by Git for this reason.

## If the game is on Archive.org

Give the agent the exact item link and exact IPA file name. Do not ask the
agent to search for a copy.

After the agent confirms that exact name is an original file in the live item
metadata, it may download only that file into `tapHLE_apps/`. That directory is
ignored by Git and is the only approved local location for Archive-backed test
apps.

Before the agent opens, inspects, or runs the IPA, it must first match:

- the Archive.org item and file name;
- whether the rightsholder still sells or officially offers the game.

Only after the live metadata confirms the exact original filename may the
agent read the app identity with `tapHLE --info` or run it. The agent records a
locally computed SHA-256 so later runs can confirm they used the same bytes.

If the item or filename does not match, stop. Do not use a different local copy
because it looks close. The full rules are in `compatibility/README.md`.

## Start the work

1. Open a [Start work on a game issue](https://github.com/ephun/tapHLE/issues/new?template=game_target.yml).
   This helps people avoid doing the same work twice. It does not put the game
   in a queue for someone else.
2. Open your tapHLE folder in your coding agent.
3. Replace the bracketed parts below, then paste the prompt into the agent.

```text
I want to improve tapHLE support for [game title and exact version] on Windows.

Exact Archive.org item URL: [URL, or "none"]
Exact Archive.org IPA file name: [file name, or "none"]
Local IPA path: [path, or "download the exact named original to tapHLE_apps/"]

Read AGENTS.md, HELP_A_GAME.md, compatibility/README.md,
dev-docs/agent-workflow.md, and dev-docs/app-debugging-playbook.md before
changing anything.

Create or continue the branch compat/[short-game-name]. Check the compatibility
database and dev-docs/app-notes first so you do not repeat old work.

If I gave an Archive.org item, verify the exact canonical item URL and original
filename in the live metadata before opening, inspecting, or running the IPA.
If no local path was supplied, download only that exact original into
tapHLE_apps/. Record a locally computed SHA-256, then read the app identity with
tapHLE --info before composing any report. Stop if the item or filename differs.
If I wrote "none," do not search for an item and do not make an Archive-linked
database report. Ask me to confirm that I authorize use of my lawful local copy
for this task. Never commit the IPA, extracted files, save data, screenshots,
raw logs, or my private file paths.

Find the first thing that stops the game from working. Make the smallest useful
fix, add a focused test when possible, and run the repository checks. Keep
unfinished experiments on the compat branch. When a checkpoint is clean,
reproducible, tested, and its limitations are recorded, prepare it for merge to
trunk even if the game is not finished. Only add a compatibility database
report after the result works from a clean commit with the exact checked IPA.
If work continues into another session, update the sanitized app note so the
next agent does not repeat the same work.

Use simple status updates. Tell me when you need me to click, play, listen, or
describe what is on screen. Push ordinary committed work as required by
AGENTS.md, but do not force-push, create a release tag, or contact anyone unless
I ask.

Follow the commit-credit rule in AGENTS.md. For OpenAI Codex, add this trailer:
Co-authored-by: OpenAI Codex <codex@openai.com>
If you are not OpenAI Codex, state the real tool identity. Do not invent a name
or email address.
```

Keep your filled-in prompt private because it contains a path on your
computer. Do not paste it into a public issue or pull request.

## Work with the agent

The agent can build code, read logs, add tests, and make commits. You may still
need to play the game, listen to sound, or describe a screen. Short, exact
answers help.

It is normal for a game to take more than one session. Useful unfinished work
belongs on `compat/<short-game-name>`. Another agent can continue from its
commits and app note later.

## Record the result in the database

The [compatibility database](https://taphle.ephun.net/compatibility) is the
public answer to "how well does this game work?" Every rating there comes from
a real tapHLE run on its reported supported host. Recording your result is how
other people find out the game moved.

Record one when the star rating changes, in either direction — a game that got
worse is worth knowing about too. Do not record one when a rerun just repeats a
rating that is already listed for the same tapHLE commit; that only makes work
for the moderator.

Record *every* step. A game you take from one star to three needs a report at
two as well as at three. It is tempting to skip the first when the next one
looks close, and that is exactly how a step goes missing — and a step that was
never recorded cannot be added later, because a report says the game was run and
rated at that commit, which nobody can honestly say afterwards.

Who records it depends on who did the testing:

- **You tested it yourself.** Sign in at the database with your GitHub account
  and fill in the form. Nothing else is needed, and you never have to ask
  anyone's permission.
- **Your agent did the work.** Your agent records it, not you. It needs an API
  token to do that, so ask for one in your
  [Start work on a game issue](https://github.com/ephun/tapHLE/issues/new?template=game_target.yml)
  and the maintainer will issue one tied to your agent.

The database records what produced each result — a person, an agent, or
automatic reporting — and that is the main reason a rating there can be trusted.
So an agent's finding is recorded as an agent's finding. Do not submit your
agent's work under your own name as though you had played the game.

A four- or five-star rating always requires a human to have actually played it.
An agent may claim at most three.

Put your agent's token in a file at `~/.taphledb-token` (on Windows,
`C:\Users\<your-username>\.taphledb-token`) and tell the agent it is there. That
file must stay off GitHub — never paste the token into an issue, a pull request,
a commit, or a chat log. The token only lets the agent submit on its own; it does
not raise the star limit and it does not skip moderation.

The database stores the rating and a one-line note of where the game stopped at
that moment, which makes each report a dated snapshot. Where a game stops *right
now*, and why, lives in `dev-docs/app-notes/<game>.md` on its compatibility
branch. Look there first when picking up work someone else started.

## Send the work back

Ask the agent to run the checks in `AGENTS.md` and explain what was really
tested. Then open a pull request from the compatibility branch.

A good pull request says:

- what changed;
- the exact game version tested;
- what now works;
- what still does not work;
- which checks passed; and
- which coding agent helped.

Partial progress is welcome when it is clearly described. Do not claim that a
game or feature works until it was tested on the claimed host from the commit
in the pull request.
