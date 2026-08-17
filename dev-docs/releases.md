# Versioning and Windows releases

tapHLE uses Semantic Versioning for its own release line. The inherited
`0.2.3` version belongs to the upstream starting point; the first tapHLE
prerelease is `0.3.0-alpha.1`, and the first stable tapHLE release will be
`0.3.0`.

## Versions and channels

- `trunk` is the preview channel. Preview builds are identified by their Git
  commit and are not numbered releases.
- Numbered prereleases use `alpha.N`, then `beta.N` when broader testing is
  appropriate, and `rc.N` only for builds believed ready to become stable.
- While the project is below 1.0, increment the minor version for a meaningful
  user-facing compatibility or emulator capability milestone. Increment the
  patch version for fixes to an existing numbered release.
- Reserve 1.0 for a dependable Windows distribution with established release,
  configuration, save-data, and compatibility expectations.

## The first release is on hold

**The first numbered tapHLE release waits for the new GUI.** Until that ships,
the trigger below does not fire, however full `Unreleased` gets. This is a
maintainer decision recorded here so that the rule is visibly held rather than
quietly not working, and `dev-scripts/release-readiness.ps1` reports it as a
blocker for the same reason.

The reasoning is that a first release is the one release that gets looked at as
a statement of what the project is. Everything after it is an increment against
that baseline. Shipping `0.3.0-alpha.1` as a command-line emulator would set
the baseline in the wrong place.

This is a hold on the *first* release only. When the GUI ships, lift it by
flipping the flag at the top of `release-readiness.ps1` and deleting this
section; the mechanical trigger below then applies from that point on and is
not subject to further judgement calls.

## When to cut one

The trigger is the changelog, not a commit count and not a judgement call about
significance:

**If `## Unreleased` in `CHANGELOG.md` has at least one user-visible entry and
`trunk` is green, cut a prerelease before starting the next body of work.**

Run `dev-scripts/release-readiness.ps1` to evaluate this. It reports MET or NOT
MET with reasons and exits non-zero when a release should not be cut, so the
answer does not depend on anyone remembering to look. Check it after merging a
batch of work; `-Quick` skips the test suites for a fast look while a build is
otherwise occupied, and says that it cannot report readiness on its own.

That is the whole rule. It is deliberately mechanical, because the previous
wording — a "meaningful milestone" — had no edge, and something with no edge
never fires: the project reached hundreds of commits and an untouched
`Unreleased` heading without a single release. A rule that depends on deciding
whether work was important enough will always lose to the next piece of work.

Consequences worth stating so the rule is not quietly softened:

- An `alpha.N` is cheap and is meant to be. Bump `N` and cut another; there is
  no cost to a prerelease that turns out to be a small one, and a large cost to
  a backlog nobody can summarise.
- If `Unreleased` is empty, there is nothing to release. Refactors, tests,
  tooling and documentation legitimately produce no entry, and a period with no
  release is the correct outcome rather than a missed one.
- Do not batch several capabilities into one release to make it look
  substantial. The changelog records what happened; the version number is a
  label, not a verdict.

The version *number* still follows the rules above: within `0.3.0`, successive
prereleases increment `alpha.N`; a meaningful capability milestone increments the
minor version and restarts at `alpha.1`.

## Release notes are the changelog

The body of a published release is the `CHANGELOG.md` section for that version,
used as it stands. Do not write release notes separately.

Two hand-maintained descriptions of the same release drift, and the one nobody
reads is maintained worst. Keeping a single source also means the entries are
written on the branch that earned them, while the reasoning is still to hand,
rather than reconstructed from the log at tag time.

At release, rename `## Unreleased` to `## <version> - YYYY-MM-DD` — the exact
form `dev-scripts/release_version.py` validates, ASCII hyphen included — open a
fresh empty `## Unreleased` above it, and paste that section into the published
release. If a release would have no changelog section, that is a sign it should
not be published.

Do not put app names, upstream revisions, dates, or a permanent `tap` suffix in
the Cargo version. Compatibility records already identify exact app versions
and emulator commits. Release notes should record the upstream base when that
provenance is useful.

## Tag and artifact names

The fork-specific annotated tag is the source of release identity:

```text
taphle-v0.3.0-alpha.1
taphle-v0.3.0-rc.1
taphle-v0.3.0
```

The `taphle-` namespace prevents imported upstream tags from being mistaken for
tapHLE releases. The corresponding user-facing version omits that namespace,
for example `v0.3.0-alpha.1`. The Windows archive is named:

```text
tapHLE-v0.3.0-alpha.1-Windows-x86_64.zip
```

Windows x86_64 is the only release artifact today. Linux and macOS are
intended desktop platforms that nothing has been released for; Android is not a
release target. `packaging.md` records what each would need.

A release artifact contains both programs: `tapHLE-gui`, the desktop frontend,
and `tapHLE`, the emulator it launches. A Windows release should also carry the
installer built from `dev-scripts/tapHLE.iss`, which has been written but not
yet built — do not publish one without checking its shortcut, its uninstall
entry and an upgrade over an existing installation.

The repository pins its Rust compiler, Clippy, and Rustfmt version in
`rust-toolchain.toml`. Update that file deliberately, run the full lint and
test suite with the new toolchain, and record any required source changes in a
normal reviewed commit. Release builds must not depend on whichever stable
toolchain happened to be installed on a runner that day.

## Release requirements

A numbered release must:

1. come from the exact current `trunk` commit, never directly from `compat/*`;
2. have a clean worktree and an exact Cargo version, changelog heading, and
   `taphle-v<version>` tag;
3. pass repository policy, formatting, unit/integration tests, and the release
   Windows build in CI;
4. contain the executable, runtime libraries/fonts, default and user option
   templates, README/changelog, and license text; and
5. avoid claims broader than the exact committed compatibility evidence.

An alpha may have incomplete game compatibility. Its release notes must state
the useful supported milestones and important remaining limitations. Do not
move or reuse a published tag; make a new version for every replacement.

## Maintainer release procedure

Agents may prepare and validate a release commit, but creating and pushing the
annotated tag requires explicit maintainer authorization.

1. Update the workspace version in `Cargo.toml` and regenerate `Cargo.lock`.
2. Turn the `CHANGELOG.md` unreleased section into an exact
   `## <version> - YYYY-MM-DD` heading, then create a fresh Unreleased section.
3. Validate the intended tag and archive name:

   ```powershell
   python dev-scripts/release_version.py check-tag taphle-v0.3.0-alpha.1
   python dev-scripts/release_version.py archive-name
   ```

4. Run the checks in `AGENTS.md`, commit, push `trunk`, and wait for its Windows
   workflow to pass.
5. From that exact clean `trunk` commit, create and push an annotated tag:

   ```powershell
   git tag -a taphle-v0.3.0-alpha.1 -m "tapHLE v0.3.0-alpha.1"
   git push origin refs/tags/taphle-v0.3.0-alpha.1
   ```

6. The tag workflow revalidates the annotated tag/version, changelog heading,
   and exact current `trunk` commit; rebuilds and tests Windows; verifies
   release identity and the absence of tracked source modifications; and
   creates the full ZIP and SHA-256 file.
7. The workflow creates a draft GitHub prerelease or stable release. Inspect
   the draft ZIP against its `.sha256` file, replace the placeholder body with
   curated notes from the changelog, and then publish it manually. Treat a
   workflow failure as a failed release attempt; fix forward with a new version
   instead of retagging a published release.

Release branches are unnecessary while only one release line is maintained.
Add one only when a real need exists to patch an older stable line while newer
development continues on `trunk`.
