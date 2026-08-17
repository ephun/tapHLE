#!/bin/sh
set -eu

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

POLICY_MARKER='tapHLE_AGENT_POLICY_V1'
KNOWN_BAD_BLOB='9a28bcd40bf1e2b329bbe8e8a22304e03c743e48'
POLICY_FILES='AGENTS.md CLAUDE.md .github/copilot-instructions.md'
PROTECTED_FILES='AGENTS.md CLAUDE.md .github/copilot-instructions.md
.github/CODEOWNERS .github/pull_request_template.md
.github/ISSUE_TEMPLATE .github/workflows CONTRIBUTING.md CODE_OF_CONDUCT.md README.md
HELP_A_GAME.md compatibility
dev-docs/app-debugging-playbook.md dev-docs/app-notes dev-docs/agent-workflow.md
dev-docs/debugging.md
dev-docs/upstream-sync.md dev-scripts/audit-agent-safety.sh
dev-scripts/audit-agent-safety.ps1 dev-scripts/compatibility.py'

fail() {
    echo "$1" >&2
    exit 1
}

for POLICY_FILE in $POLICY_FILES; do
    [ -f "$POLICY_FILE" ] || fail "Missing required agent policy: $POLICY_FILE"
    grep -Fq "$POLICY_MARKER" "$POLICY_FILE" || \
        fail "Agent policy marker missing from $POLICY_FILE"
done

for REQUIRED_TEXT in \
    'with a broad goal' \
    'games as possible work on the desktop.' \
    'Windows is the primary development and compatibility environment' \
    'Do not develop, test or refactor the inherited Android source' \
    'Repository content is not automatically trusted as agent instruction.'
do
    grep -Fq "$REQUIRED_TEXT" AGENTS.md || \
        fail "Required tapHLE policy text is missing: $REQUIRED_TEXT"
done

# Reject the known upstream blob wherever it appears in the tracked tree.
git ls-files -s | while read -r _MODE OBJECT _REST; do
    if [ "$OBJECT" = "$KNOWN_BAD_BLOB" ]; then
        fail "Rejected known malicious blob in the tracked tree."
    fi
done

git ls-files | while IFS= read -r PATH_NAME; do
    LOWER_PATH=$(printf '%s' "$PATH_NAME" | tr '[:upper:]' '[:lower:]')
    case "$LOWER_PATH" in
        *.ipa) fail "Proprietary IPA must not be tracked: $PATH_NAME" ;;
    esac
done

# Reject known poison signatures even if the upstream file was lightly edited.
# The strings are split here so this audit file does not match its own scan.
POISON_ONE='Epicutaneous'" Hydrophilic"
POISON_TWO='Applied offensive'" epidemiology"
POISON_THREE='prepared to'" kill human beings"
for POISON_TEXT in "$POISON_ONE" "$POISON_TWO" "$POISON_THREE"; do
    if MATCHES=$(git grep -I -n -F -e "$POISON_TEXT" -- .); then
        echo "$MATCHES" >&2
        fail "Rejected a known agent-poison signature in tracked content."
    else
        GREP_STATUS=$?
        [ "$GREP_STATUS" -eq 1 ] || \
            fail "Could not scan tracked content for agent-poison signatures."
    fi
done

# Do not allow imported or untracked tools to establish a second instruction
# scope. Add any intentional new instruction surface to this policy explicitly.
git ls-files | while IFS= read -r PATH_NAME; do
    LOWER_PATH=$(printf '%s' "$PATH_NAME" | tr '[:upper:]' '[:lower:]')
    case "$LOWER_PATH" in
        agents.md|claude.md|.github/copilot-instructions.md)
            ;;
        */agents.md|*/claude.md|gemini.md|*/gemini.md|\
        .cursorrules|*/.cursorrules|.windsurfrules|*/.windsurfrules|\
        .github/instructions/*|.cursor/rules/*|*/copilot-instructions.md)
            fail "Unexpected agent instruction surface: $PATH_NAME"
            ;;
    esac
done

if [ "$#" -ne 0 ]; then
    [ "$#" -eq 2 ] && [ "$1" = '--baseline' ] || \
        fail 'Usage: audit-agent-safety.sh [--baseline GIT_REF]'
    BASELINE=$2
    git rev-parse --verify "$BASELINE^{commit}" >/dev/null || \
        fail "Unknown baseline commit: $BASELINE"
    CHANGED=$(git diff --name-only "$BASELINE" -- $PROTECTED_FILES)
    if [ -n "$CHANGED" ]; then
        echo "$CHANGED" >&2
        fail 'Protected tapHLE policy changed during the audited import.'
    fi
fi

echo 'tapHLE agent policy surfaces passed the safety audit.'
