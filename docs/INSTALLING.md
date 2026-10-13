# Install thepeer

Choose a published release for normal use, a draft release for pre-publication
testing, or a source build for development. Use the CLI and Android app from the
same release or source revision.

## Published releases

### CLI

Install the latest normal release:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/benjaminpreiss/thepeer.stream/main/scripts/install.sh | sh
```

Review the [installer](../scripts/install.sh) before running it. It verifies the
archive against the release's `SHA256SUMS`, installs `~/.local/bin/thepeer` without
sudo, and retains the archive with licenses under `~/.local/share/thepeer.stream/`.
Add `~/.local/bin` to your `PATH` if needed.

Supported binaries: Linux x86_64 with glibc 2.35+, and macOS 15+ on Apple silicon
or Intel. macOS binaries are not notarized. Checksums detect mismatched downloads;
they are not independent publisher authentication.

For a specific published version or prerelease, set its tag explicitly:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/benjaminpreiss/thepeer.stream/main/scripts/install.sh | THEPEER_VERSION=v0.1.0 sh
```

Replace `v0.1.0` with the desired tag. The default installer excludes prereleases
and drafts; an explicit version supports published prereleases, **not drafts**.

### Android app

Open [GitHub Releases](https://github.com/benjaminpreiss/thepeer.stream/releases)
and download `thepeer.stream-TAG-android-arm64.apk` from the chosen release.
Open it on the phone and approve installation from that source when Android asks.
The app requires an arm64 phone running Android 8.0 or newer.

The application ID is `stream.thepeer`; its displayed name is **thepeer**. Earlier
prototype APKs used `dev.webp2p.controllerprobe`. The new ID installs as a separate
app, not an update to that prototype. You can uninstall the old app after switching;
uninstalling removes its local app data. Existing published APKs keep their original
ID—the change takes effect in newly built APKs.

The release also includes an Android license archive and `SHA256SUMS`. To verify
assets on your computer, download all release assets into one directory and run:

```sh
# From the directory containing SHA256SUMS and all five release assets:
shasum -a 256 --check SHA256SUMS
```

Do not install if verification fails. On Linux, `sha256sum --check SHA256SUMS`
is an alternative. Keep the Android license archive with the APK.

## Draft releases: pre-publication testing

Drafts are not public downloads. You need a GitHub account with permission to view
the repository's drafts, normally repository write access. Merely knowing the tag
or having read access to the public repository is not sufficient.

Install [GitHub CLI](https://cli.github.com/) and authenticate with `gh auth login`.
You can also download assets through GitHub's Releases page when signed into an
account with the necessary access. A failed workflow may leave missing assets;
wait for the complete draft before installing.

### Download, verify and install the draft CLI

Run the following in Bash or Zsh on a supported computer. Replace `v0.1.0` with
the draft's tag. This downloads all five assets so every checksum can be checked,
installs the CLI, and keeps the APK and license archives for review. It does not
publish the draft or execute the downloaded CLI.

```sh
(
  set -eu
  version=v0.1.0
  repo=benjaminpreiss/thepeer.stream
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64) platform=linux-x86_64 ;;
    Darwin:arm64) platform=macos-arm64 ;;
    Darwin:x86_64) platform=macos-x86_64 ;;
    *) echo 'Unsupported CLI platform' >&2; exit 1 ;;
  esac

  download=$(mktemp -d "$HOME/thepeer-draft.XXXXXX")
  printf 'Draft assets will be kept in: %s\n' "$download"
  gh release download "$version" --repo "$repo" --dir "$download"
  (cd "$download" && shasum -a 256 --check SHA256SUMS)

  name="thepeer.stream-$version-$platform"
  tar -xzOf "$download/$name.tar.gz" "$name/thepeer" > "$download/thepeer"
  test -s "$download/thepeer"
  mkdir -p "$HOME/.local/bin"
  install -m 755 "$download/thepeer" "$HOME/.local/bin/thepeer"
  printf 'Installed draft CLI. Keep its archive for licenses: %s\n' "$download/$name.tar.gz"
  printf 'Android APK: %s/thepeer.stream-%s-android-arm64.apk\n' "$download" "$version"
)
```

The subshell stops on download/checksum/extraction errors before installation.
On Linux without `shasum`, substitute `sha256sum --check SHA256SUMS` for the checksum
command. Add `~/.local/bin` to `PATH` if needed. This replaces any CLI already at
that location; rerun the regular installer to return to the latest published release.
Retain the verified archive for its licenses and build metadata.

### Install the draft Android app

Transfer the verified APK from the printed download directory to the phone, open
it and allow installation. Keep its matching `android-arm64-licenses.tar.gz` archive.
Alternatively, with Android platform tools installed and USB debugging authorized:

```sh
adb install -r /path/to/thepeer.stream-v0.1.0-android-arm64.apk
```

Draft APKs produced by the release workflow use the release signing key. Android
updates require the same signing identity and a compatible version code; an older
build may be rejected. Debug source builds use a separate ID, `stream.thepeer.debug`, and can coexist
with release builds.

Draft status is not a quality guarantee. Test Direct, Relay and Stop using the
[physical testing guide](HUMAN-TESTING.md) before promoting a draft. Release
creation and publication are documented separately in [releasing](RELEASING.md).

## Build and install from source

Follow [Build thepeer.stream from source](BUILDING.md) for prerequisites, cloning,
the pinned Rust toolchain, Android SDK/NDK setup and exact build commands.

- **CLI:** `bash scripts/build-helper.sh` produces `target/build/debug/thepeer`
  unless you override the target directory. Run that executable directly, or install
  it after a successful build:

  ```sh
  mkdir -p "$HOME/.local/bin"
  install -m 755 target/build/debug/thepeer "$HOME/.local/bin/thepeer"
  ```

- **Android:** build/stage the native library with `native-build.sh`, then build
  and install the debug APK through Android Studio or the Gradle/ADB commands in
  the [Android source-build steps](BUILDING.md#3-build-and-install-the-phone-app).
  This installs **thepeer (Debug)** (`stream.thepeer.debug`), with a `-debug` version
  suffix and a local debug signing key. The launcher, app title and notification
  distinguish it from **thepeer** (`stream.thepeer`); both can be installed together.

Use the project's build scripts rather than an unpatched root `cargo build`.
Do not bypass native source/artifact receipt checks. If testing an exact release,
check out its tag before building both components.

Once installed, follow the [Start instructions](../README.md#start).
