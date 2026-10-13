#!/bin/sh
# Install a published helper; never run the downloaded executable here.
set -eu
repo=https://github.com/benjaminpreiss/thepeer.stream
fail() { printf '%s\n' "thepeer.stream: $*" >&2; exit 1; }
for tool in curl tar awk grep mktemp; do
  command -v "$tool" >/dev/null 2>&1 || fail "Missing prerequisite: $tool"
done
case "$(uname -s):$(uname -m)" in
  Linux:x86_64) platform=linux-x86_64 ;;
  Darwin:arm64) platform=macos-arm64 ;;
  Darwin:x86_64) platform=macos-x86_64 ;;
  *) fail 'Supported platforms: Linux x86_64 and macOS arm64/x86_64.' ;;
esac
if command -v sha256sum >/dev/null 2>&1; then
  checksum() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
  checksum() { shasum -a 256 "$1" | awk '{print $1}'; }
else
  fail 'Install sha256sum or shasum first.'
fi
version=${THEPEER_VERSION:-}
if [ -z "$version" ]; then
  url=$(curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL -o /dev/null -w '%{url_effective}' "$repo/releases/latest") || fail 'No latest release available; publish a release or set THEPEER_VERSION.'
  version=${url##*/}
fi
case "$version" in ''|*[!A-Za-z0-9.-]*) fail 'Invalid release version.' ;; esac
printf '%s\n' "$version" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9][A-Za-z0-9.-]*)?$' || fail 'Invalid release version.'
name="thepeer.stream-$version-$platform"
archive="$name.tar.gz"
prefix=${THEPEER_PREFIX:-"$HOME/.local"}
[ -n "$prefix" ] || fail 'Empty install prefix.'
tmp=$(mktemp -d)
staged=
cleanup() { rm -rf "$tmp"; [ -z "$staged" ] || rm -f "$staged"; }
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
base="$repo/releases/download/$version"
curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS"
curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL "$base/$archive" -o "$tmp/$archive"
expected=$(awk -v name="$archive" '$2 == name {print $1}' "$tmp/SHA256SUMS")
[ "${#expected}" -eq 64 ] || fail 'Missing or duplicate archive checksum.'
printf '%s\n' "$expected" | grep -Eq '^[0-9a-f]{64}$' || fail 'Invalid checksum.'
[ "$(checksum "$tmp/$archive")" = "$expected" ] || fail 'Checksum mismatch; nothing installed.'
# Extract only this exact member to stdout, not arbitrary archive paths to disk.
tar -xzOf "$tmp/$archive" "$name/thepeer" > "$tmp/thepeer"
[ -s "$tmp/thepeer" ] || fail 'Release archive contains no helper.'
mkdir -p "$prefix/bin" "$prefix/share/thepeer.stream/$version"
# Retain the verified archive, including its license texts and build metadata.
cp "$tmp/$archive" "$prefix/share/thepeer.stream/$version/$archive"
staged=$(mktemp "$prefix/bin/.thepeer.XXXXXX")
cp "$tmp/thepeer" "$staged"
chmod 755 "$staged"
mv -f "$staged" "$prefix/bin/thepeer"
staged=
printf 'Installed %s to %s/bin/thepeer\n' "$version" "$prefix"
printf 'Licenses and build metadata: %s/share/thepeer.stream/%s/%s\n' "$prefix" "$version" "$archive"
case ":$PATH:" in
  *":$prefix/bin:"*) ;;
  *) printf 'Add %s/bin to your PATH to run thepeer.\n' "$prefix" ;;
esac
