# Install thepeer

Use matching CLI versions on both computers, or a CLI and Android app from the
same release or source revision. Choose a [published release](#published-releases),
[authenticated draft](#test-a-draft-release), or [source build](#build-from-source).

## Published releases

### CLI

Install the latest **normal** release on Linux x86_64 (glibc 2.35+) or macOS 15+
(Apple silicon or Intel):

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/benjaminpreiss/thepeer.stream/main/scripts/install.sh | sh
```

Review the [installer](../scripts/install.sh) first. It verifies the release's
`SHA256SUMS` and installs `~/.local/bin/thepeer` without sudo. Add `~/.local/bin`
to your `PATH` if needed. It retains the archive and license notices under
`~/.local/share/thepeer.stream/`.

To install a **published** prerelease or a specific release, replace `TAG` with
its tag:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/benjaminpreiss/thepeer.stream/main/scripts/install.sh | THEPEER_VERSION=TAG sh
```

The default installer excludes prereleases and drafts. A pinned published
prerelease installs like any other release. To keep your stable binary, set
`THEPEER_PREFIX` to a separate directory and run the test binary by its full path.
The curl installer cannot download draft assets.

### Android

Download `thepeer.stream-TAG-android-arm64.apk` from the chosen
[GitHub Release](https://github.com/benjaminpreiss/thepeer.stream/releases),
then open it on an arm64 phone running Android 8.0 or later. Approve installation
when Android prompts. Keep its `android-arm64-licenses.tar.gz` archive.

To verify downloads, put `SHA256SUMS` and all five release assets in one directory
and run `shasum -a 256 --check SHA256SUMS` there (`sha256sum --check` on Linux).
Checksums detect mismatched files; they do not authenticate the publisher.

Release APKs use application ID `stream.thepeer`. Android updates require the same
signing key and a higher version code. An installed RC can prevent reinstalling
an older stable APK; use a separate phone or a debug source build for coexistence.

## Test a draft release

Draft assets require a GitHub account authorized to view repository drafts.
Authenticate with [GitHub CLI](https://cli.github.com/) (`gh auth login`), then
set `version` to the draft tag below. Check that all assets are present before
installing. This example leaves any stable CLI at `~/.local/bin/thepeer` intact:

```sh
(
  set -eu
  version=TAG
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64) platform=linux-x86_64 ;;
    Darwin:arm64) platform=macos-arm64 ;;
    Darwin:x86_64) platform=macos-x86_64 ;;
    *) echo 'Unsupported CLI platform' >&2; exit 1 ;;
  esac
  download=$(mktemp -d "$HOME/thepeer-draft.XXXXXX")
  gh release download "$version" --repo benjaminpreiss/thepeer.stream --dir "$download"
  (cd "$download" && shasum -a 256 --check SHA256SUMS)
  name="thepeer.stream-$version-$platform"
  mkdir -p "$HOME/thepeer-draft/bin"
  tar -xzOf "$download/$name.tar.gz" "$name/thepeer" > "$download/thepeer"
  install -m 755 "$download/thepeer" "$HOME/thepeer-draft/bin/thepeer"
  printf 'Test CLI: %s\nDraft assets: %s\n' "$HOME/thepeer-draft/bin/thepeer" "$download"
)
```

Run that CLI by its full path on **both** computers. The draft APK is in the
printed assets directory; transfer it to the phone and install it, or use
`adb install -r /path/to/thepeer.stream-TAG-android-arm64.apk`. Retain the
matching license archive. Draft status is not a quality guarantee: follow the
[physical testing protocol](HUMAN-TESTING.md) before publishing.

## Build from source

Follow [BUILDING.md](BUILDING.md) to build the CLI and the Android debug app.
Run `target/build/debug/thepeer` directly, or install it locally:

```sh
mkdir -p "$HOME/.local/bin"
install -m 755 target/build/debug/thepeer "$HOME/.local/bin/thepeer"
```

The debug Android app (`stream.thepeer.debug`) can coexist with the release app.
Build both components from the same tag or revision. See [How to](../README.md#how-to)
for usage and [RELEASING.md](RELEASING.md) for publication.
