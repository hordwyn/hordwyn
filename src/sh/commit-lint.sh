#!/bin/sh
# Rejects commits whose message carries AI attribution or non-ASCII characters (KICKOFF rules 3, 10).
# usage: commit-lint.sh [<rev-range>]   (default: all commits reachable from HEAD)
set -eu
range="${1:-HEAD}"
pattern='(?i)co-authored-by:.*(claude|gpt|copilot|ai\b)|generated with|claude\.ai|anthropic'
status=0
for c in $(git rev-list "$range"); do
  msg="$(git log -1 --format=%B "$c")"
  if printf '%s' "$msg" | grep -qP "$pattern"; then
    echo "commit $c: message contains AI attribution" >&2
    status=1
  fi
  if printf '%s' "$msg" | LC_ALL=C grep -qP '[^\x00-\x7F]'; then
    echo "commit $c: message contains non-ASCII characters" >&2
    status=1
  fi
done
exit "$status"
