# Report whether the conditions for a numbered release are met.
#
# The release trigger in dev-docs/releases.md is deliberately mechanical, but a
# rule nobody evaluates is the same as no rule: this project reached hundreds of
# commits, an untouched Unreleased heading, and zero published releases. This
# turns the rule into something you can run.
#
#   .\dev-scripts\release-readiness.ps1          # full check
#   .\dev-scripts\release-readiness.ps1 -Quick   # skip the test suites
#
# Exit code is 0 when a release should be cut, 1 when it should not, so it can
# gate something later. -Quick is for a fast look while other work holds the
# build; it cannot report readiness on its own and says so.

[CmdletBinding()]
param([switch]$Quick)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

# The first numbered release waits for the new GUI; see "The first release is
# on hold" in dev-docs/releases.md. Set this to $false when the GUI ships and
# delete that section - after the first release the changelog trigger applies
# on its own. A held release is stated rather than left to look like a rule
# that mysteriously never fires.
$FirstReleaseHeldForGui = $true

$blockers = @()
$notes = @()

function Add-Blocker($text) { $script:blockers += $text }
function Add-Note($text) { $script:notes += $text }

# Windows PowerShell 5.1 wraps each stderr line from a native program in an
# ErrorRecord, which `$ErrorActionPreference = 'Stop'` above then treats as
# terminating - so `cargo test 2>&1` aborts this script the moment cargo
# prints a "Compiling ..." line, which it does on any build that is not fully
# cached. Capturing through here keeps stderr merged (the test summary is what
# we want to read) without letting it end the run.
function Invoke-Capture {
    param([string[]]$CargoArgs)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & cargo @CargoArgs 2>&1 | Out-String } finally { $ErrorActionPreference = $previous }
}

# 0. The standing hold, checked before anything else so a held release is never
#    reported as ready no matter how green the rest of the run is.
if ($FirstReleaseHeldForGui) {
    Add-Blocker 'The first release is held until the new GUI ships (dev-docs/releases.md).'
}

# 1. Is there anything to release? This is the trigger, not a formality: an
#    empty Unreleased section means the correct action is to do nothing.
$changelog = Get-Content 'CHANGELOG.md' -Raw
$unreleased = [regex]::Match($changelog, '(?ms)^##\s+Unreleased[^\r\n]*\r?\n(.*?)(?=^##\s|\z)')
if (-not $unreleased.Success) {
    Add-Blocker 'CHANGELOG.md has no "## Unreleased" section.'
} else {
    $entries = ($unreleased.Groups[1].Value -split "`n" | Where-Object { $_ -match '^\s*-\s+\S' }).Count
    if ($entries -eq 0) {
        Add-Blocker 'CHANGELOG.md "## Unreleased" has no entries - there is nothing to release, which is a valid state rather than a problem.'
    } else {
        Add-Note "$entries changelog entries are awaiting release."
    }
}

# 2. Requirement 1 and 2: exactly the current trunk commit, clean worktree.
$branch = (git rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne 'trunk') { Add-Blocker "On branch '$branch'; a release must come from trunk." }

if ((git status --porcelain).Length -gt 0) { Add-Blocker 'Worktree is not clean.' }

git fetch origin --quiet 2>$null
$ahead = (git rev-list --count 'origin/trunk..HEAD' 2>$null)
$behind = (git rev-list --count 'HEAD..origin/trunk' 2>$null)
if ($behind -ne '0') { Add-Blocker "trunk is $behind commits behind origin/trunk." }
if ($ahead -ne '0') { Add-Note "trunk is $ahead commits ahead of origin; push before tagging." }

# 3. The version must be a real prerelease and must not already be tagged.
$version = (Select-String -Path 'Cargo.toml' -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Add-Note "Cargo version is $version."
if (git tag --list "taphle-v$version") {
    Add-Blocker "taphle-v$version already exists; bump the version rather than moving a published tag."
}

# 4. Requirement 3: formatting and both test suites.
if ($Quick) {
    Add-Note 'Skipped formatting and tests (-Quick). This run cannot report readiness.'
} else {
    cargo fmt --all -- --check *> $null
    if ($LASTEXITCODE -ne 0) { Add-Blocker 'cargo fmt --check fails.' }

    # Matched on the success line rather than on a failure word: a *passing*
    # summary reads "0 failed", and PowerShell's -match is case-insensitive, so
    # looking for 'FAILED' reports every green run as a red one. Absence of the
    # success line also covers the suite failing to build at all, which is what
    # a missing 'test result:' means.
    $unit = Invoke-Capture @('test', '--release', '--lib')
    if ($unit -match 'test result: ok\.') {
        Add-Note 'Unit tests pass.'
    } else {
        Add-Blocker 'Unit tests fail.'
    }

    # Named separately because releases.md requires integration tests too, and
    # this is the one that has been failing.
    $integration = Invoke-Capture @('test', '--release', '--test', 'integration')
    if ($integration -notmatch 'test result: ok\.') {
        Add-Blocker 'Integration tests fail (releases.md requirement 3 names them explicitly).'
    } else {
        Add-Note 'Integration tests pass.'
    }
}

Write-Output ''
foreach ($n in $notes) { Write-Output "  - $n" }

if ($blockers.Count -eq 0 -and -not $Quick) {
    Write-Output ''
    Write-Output "RELEASE CONDITIONS MET for $version."
    Write-Output 'Prepare the release commit, then ask the maintainer to authorize the tag:'
    Write-Output '  - rename "## Unreleased" to "## <version> - <date>" and open a fresh empty Unreleased'
    Write-Output '  - the published release body is that changelog section, used as it stands'
    Write-Output 'Creating or pushing the tag requires explicit maintainer authorization (AGENTS.md).'
    exit 0
}

Write-Output ''
Write-Output 'RELEASE CONDITIONS NOT MET:'
foreach ($b in $blockers) { Write-Output "  x $b" }
exit 1
