#!/usr/bin/env bash
# Prepares a release: sets the version everywhere it is kept, commits that, and
# tags the commit. It does not push; it prints the two commands for that.
#
#   scripts/release.sh 0.1.2            do it
#   scripts/release.sh 0.1.2 --dry-run  check and say what would change, touch nothing
#
# The version lives in Cargo.toml, ui/package.json and ui/src-tauri/tauri.conf.json
# (plus the two lock files that record it). The Release workflow refuses a tag
# that does not match all three, so they are always changed together.
set -euo pipefail

usage() {
  echo "usage: scripts/release.sh X.Y.Z [--dry-run]" >&2
  exit 2
}

[ $# -ge 1 ] && [ $# -le 2 ] || usage
version="$1"
dry_run=false
if [ $# -eq 2 ]; then
  [ "$2" = "--dry-run" ] || usage
  dry_run=true
fi

die() {
  echo "error: $*" >&2
  exit 1
}

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] ||
  die "'$version' is not a version like 1.2.3 (no leading v; the tag adds it)"

cd "$(git rev-parse --show-toplevel)"
tag="v$version"

files=(Cargo.toml Cargo.lock ui/package.json ui/package-lock.json ui/src-tauri/tauri.conf.json)

current_cargo=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
current_npm=$(jq -r .version ui/package.json)
current_tauri=$(jq -r .version ui/src-tauri/tauri.conf.json)
[ "$current_cargo" = "$current_npm" ] && [ "$current_cargo" = "$current_tauri" ] ||
  die "the versions already disagree: Cargo.toml $current_cargo, package.json $current_npm, tauri.conf.json $current_tauri. Make them match first."

[ "$(git branch --show-current)" = "main" ] || die "not on main (releases are made from main)"
[ -z "$(git status --porcelain --untracked-files=no)" ] ||
  die "there are uncommitted changes to tracked files; commit or stash them first"
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "the tag $tag already exists here (delete it with: git tag -d $tag)"
[ "$version" != "$current_cargo" ] || die "the version is already $version"
newest=$(printf '%s\n%s\n' "$current_cargo" "$version" | sort -V | tail -n1)
[ "$newest" = "$version" ] || die "$version is older than the current version $current_cargo"

echo "Release $version (currently $current_cargo), tagging $tag on $(git rev-parse --short HEAD)."
if $dry_run; then
  echo "Dry run: would set the version in ${files[*]}, commit, and tag. Nothing was changed."
  exit 0
fi

# If anything fails before the commit, put the files back the way they were.
committed=false
restore() {
  if ! $committed; then
    echo "Failed; restoring the version files." >&2
    git checkout -- "${files[@]}"
  fi
}
trap restore EXIT

# First match only: the workspace version in Cargo.toml, the top-level "version" in the JSON.
sed -i "0,/^version = \".*\"/s//version = \"$version\"/" Cargo.toml
sed -i "0,/\"version\": \".*\"/s//\"version\": \"$version\"/" ui/src-tauri/tauri.conf.json
# npm changes package.json and package-lock.json together, without touching git.
(cd ui && npm version "$version" --no-git-tag-version --allow-same-version >/dev/null)
# Refresh the owl crates' entries in Cargo.lock (no other package changes).
cargo update --workspace --quiet

# Check what was written, the same way the Release workflow does.
[ "$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)" = "$version" ] || die "Cargo.toml was not updated"
[ "$(jq -r .version ui/package.json)" = "$version" ] || die "package.json was not updated"
[ "$(jq -r .version ui/src-tauri/tauri.conf.json)" = "$version" ] || die "tauri.conf.json was not updated"
[ "$(jq -r .version ui/package-lock.json)" = "$version" ] || die "package-lock.json was not updated"

git add "${files[@]}"
git commit --quiet -m "chore: release $version"
committed=true
git tag -a "$tag" -m "Owl $version"

echo
echo "Done. Committed 'chore: release $version' and tagged it $tag."
echo "Check it with:  git show --stat HEAD   and   git show $tag"
echo "Then publish:   git push origin main && git push origin $tag"
echo "To undo before pushing:  git tag -d $tag && git reset --hard HEAD~1"
