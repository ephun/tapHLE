## Outcome

Describe the user-visible result. Name the target game and exact version when
this is compatibility work.

## Evidence and implementation

Give the reproduction, relevant log or probe, root-cause finding, and why this
is the smallest useful fix. Explain the boundary around any partial
implementation or game-specific workaround.

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] Relevant unit or TestApp test
- [ ] `cargo test -- --skip test_app`
- [ ] `cargo build --release`
- [ ] Exact target game launched on the claimed host
- [ ] `python dev-scripts/compatibility.py check`
- [ ] `bash dev-scripts/lint.sh`
- [ ] `dev-scripts/regression-sweep.ps1`, if a shared path changed

List the checks actually run, host OS/device/CPU/GPU details for game
validation, and any skipped checks with their reason.

## Documentation impact

- [ ] Platform status, settings/CLI behaviour, release policy, or the
      compatibility protocol changed — and the file that **owns** that fact
      (see the table in `docs/README.md`) was updated.
- [ ] No fact was restated in a second document instead of linked.
- [ ] No new documentation file was added without the maintainer asking.

If none applies, say "no documentation impact".

## Provenance and agents

List material public documentation, behavioral tests, or compatibly licensed
code used. Name any AI agent/tool that materially assisted and what a human
verified.

- [ ] No game binaries, assets, keys, personal data, or unauthorized links are
      included.
- [ ] No leaked/private Apple implementation material or incompatible code was
      used.
- [ ] Upstream changes were reviewed under `docs/maintaining.md`.
- [ ] Any compatibility result named the app build it was earned on, read from
      `tapHLE --info`, on a committed tapHLE revision; no dirty-worktree result
      was entered in the database.
