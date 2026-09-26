#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf 'git-bs install: %s\n' "$*" >&2
  exit 1
}

for dependency in curl git sha256sum tar mktemp install mv awk cmp mkdir rm uname; do
  command -v "$dependency" >/dev/null 2>&1 || fail "Required command not found: $dependency"
done

[[ $(uname -s) == Linux ]] || fail 'This installer supports Linux only; use Homebrew on macOS.'
case "$(uname -m)" in
  x86_64|amd64) platform=linux-x86_64 ;;
  aarch64|arm64) platform=linux-arm64 ;;
  *) fail "Unsupported Linux CPU architecture: $(uname -m)" ;;
esac

: "${HOME:?HOME is required}"
install_dir=${GIT_BS_INSTALL_DIR:-"$HOME/.local/bin"}
release_base=${GIT_BS_RELEASE_BASE_URL:-https://github.com/Bhacaz/git-bs/releases}
release_base=${release_base%/}

curl_args=(--fail --location --silent --show-error --retry 3)
latest_url=$(curl "${curl_args[@]}" --output /dev/null --write-out '%{url_effective}' "$release_base/latest") \
  || fail 'Could not find the latest GitHub release.'
case "$latest_url" in
  "$release_base"/tag/*) tag=${latest_url##*/} ;;
  *) fail "Unexpected latest release URL: $latest_url" ;;
esac
[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "Invalid release tag: $tag"

archive="git-bs-$tag-$platform.tar.gz"
temp_dir=$(mktemp -d) || fail 'Could not create a temporary directory.'
staged_file=
cleanup() {
  rm -rf -- "$temp_dir"
  if [[ -n $staged_file ]]; then rm -f -- "$staged_file"; fi
}
trap cleanup EXIT

curl "${curl_args[@]}" --output "$temp_dir/$archive" "$release_base/download/$tag/$archive" \
  || fail "Could not download $archive."
curl "${curl_args[@]}" --output "$temp_dir/SHA256SUMS" "$release_base/download/$tag/SHA256SUMS" \
  || fail 'Could not download release checksums.'

checksum=$(awk -v filename="./$archive" '$2 == filename { print $1 }' "$temp_dir/SHA256SUMS")
[[ $checksum =~ ^[0-9a-fA-F]{64}$ ]] || fail "No valid checksum found for $archive."
(
  cd "$temp_dir"
  printf '%s  %s\n' "$checksum" "$archive" | sha256sum --check --status -
) || fail "Checksum verification failed for $archive."

[[ $(tar -tzf "$temp_dir/$archive") == git-bs ]] || fail 'Unexpected archive contents.'
tar -xzf "$temp_dir/$archive" -C "$temp_dir" git-bs
[[ $("$temp_dir/git-bs" --version) == "git-bs ${tag#v}" ]] \
  || fail 'The executable version does not match the release tag.'

mkdir -p -- "$install_dir"
destination="$install_dir/git-bs"
[[ ! -d $destination ]] || fail "$destination is a directory."
if [[ -f $destination ]] && cmp -s -- "$temp_dir/git-bs" "$destination"; then
  action='Already up to date'
else
  action='Installed'
  [[ -e $destination || -L $destination ]] && action='Updated'
  staged_file=$(mktemp "$install_dir/.git-bs.XXXXXX")
  install -m 0755 "$temp_dir/git-bs" "$staged_file"
  mv -fT -- "$staged_file" "$destination"
  staged_file=
fi

shell_quote() {
  local value=$1 result="'" character index
  for ((index = 0; index < ${#value}; index++)); do
    character=${value:index:1}
    if [[ $character == "'" ]]; then
      result+="'\\''"
    else
      result+="$character"
    fi
  done
  printf "%s'" "$result"
}

git config --global --replace-all alias.bs "!$(shell_quote "$destination")" \
  || fail "Installed $destination, but could not configure the Git bs alias."

printf '%s git-bs %s at %s\n' "$action" "${tag#v}" "$destination"
printf 'Run: git bs\n'
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'For direct git-bs use, add %s to your PATH.\n' "$install_dir" ;;
esac
