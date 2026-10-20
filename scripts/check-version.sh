#!/usr/bin/env bash
# Checks that the workspace is ready to be tagged as a release.
#
#   scripts/check-version.sh            checks internal pins only
#   scripts/check-version.sh 0.1.0      also checks that 0.1.0 is the workspace version
#                                       and has a dated section in CHANGELOG.md
#
# Every internal crate in [workspace.dependencies] must pin `version = "=<workspace version>"`
# so the crates move together and a tag names one consistent set.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$root/Cargo.toml"
changelog="$root/CHANGELOG.md"

workspace_version="$(awk '
  /^\[workspace\.package\]/ { in_pkg = 1; next }
  /^\[/ { in_pkg = 0 }
  in_pkg && $1 == "version" { gsub(/"/, "", $3); print $3; exit }
' "$manifest")"

if [[ -z "$workspace_version" ]]; then
  echo "error: no version in [workspace.package] of Cargo.toml" >&2
  exit 1
fi

status=0
while IFS= read -r line; do
  name="${line%% *}"
  if [[ "$line" != *"version = \"=$workspace_version\""* ]]; then
    echo "error: $name in [workspace.dependencies] must pin version = \"=$workspace_version\"" >&2
    status=1
  fi
done < <(grep -E '^craft-[a-z0-9-]+ = \{ path = "crates/' "$manifest")

if [[ $# -ge 1 ]]; then
  wanted="$1"
  if ! [[ "$wanted" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "error: '$wanted' is not a version of the form X.Y.Z" >&2
    exit 1
  fi
  if [[ "$wanted" != "$workspace_version" ]]; then
    echo "error: asked for $wanted but Cargo.toml [workspace.package] version is $workspace_version" >&2
    status=1
  fi
  if ! grep -Eq "^## \[$(printf '%s' "$wanted" | sed 's/\./\\./g')\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" "$changelog"; then
    echo "error: CHANGELOG.md has no dated section '## [$wanted] - YYYY-MM-DD'" >&2
    status=1
  fi
fi

if [[ $status -eq 0 ]]; then
  echo "ok: workspace version $workspace_version"
fi
exit $status
