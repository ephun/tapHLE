#!/bin/sh

echo 'Checking for long comment lines…'
# Single-line comments are easy to find and they are the norm in this project,
# so only those are checked for.
# Comments not preceded by whitespace are ignored since the line length limit
# for source is longer than 80 currently.
# Comments containing URLs are ignored because wrapping those is unreasonable.
#
# The length test is done in perl rather than grep. `grep '.\{81,\}'` counts
# bytes, not characters, so a correctly wrapped 80-column line was reported as
# too long once it contained anything non-ASCII: an em dash costs three bytes,
# and this codebase uses them throughout its prose. That made the check
# disagree with the message it prints, and 75 of the 134 lines it was flagging
# were already within 80 characters. `perl -CSD` decodes UTF-8, so `length`
# counts characters. It is used instead of a locale-dependent grep or awk
# because this script runs on both the macOS and Windows CI runners, and BSD
# grep has no `-P`.
long_comments=$(grep -n '^ *\/\/' \
    -r --include '*.rs' --include '*.cpp' --include '*.hpp' --include '*.c' \
    --include '*.h' --include '*.m' \
    build.rs src tests/*.rs tests/TestApp_source \
    | grep -v 'https:\|http:' \
    | perl -CSD -ne 'if (/^[^:]*:[0-9]+:(.*)$/) { print if length($1) > 80 }')
if [ -n "$long_comments" ]; then
    printf '%s\n' "$long_comments"
    printf '\e[31m''Overly long comment lines found. Please wrap comment lines to 80 characters.''\e[0m\n'
    exit 1
fi
printf '\e[32mNone found.\e[0m\n'

set -ex

# "--deny warnings" ensures that warnings result in a non-zero exit status.
cargo $@ clippy -- --deny warnings
# "--document-private-items" has to be added again so the flag from
# .cargo/config.toml isn't overridden
RUSTDOCFLAGS="--deny warnings --document-private-items" cargo doc
