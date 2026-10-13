# Draft releases

`.github/workflows/release.yaml` runs **only by manual dispatch on a pushed version
tag**. It reuses the normal tests, builds three helpers and a signed Android APK,
and creates a **draft**, never a published release. No relay is contacted or deployed.
The workflow must be on the default branch before GitHub offers manual dispatch.

## One-time operator setup

These steps need repository administration and signing credentials. The agent does
not create/upload keys, configure GitHub environments or dispatch releases for you.

1. Install/authenticate [GitHub CLI](https://cli.github.com/) with permission to
   manage secrets in `benjaminpreiss/thepeer.stream` (`gh auth login`).
2. Open [repository environments](https://github.com/benjaminpreiss/thepeer.stream/settings/environments)
   and create an environment named **release**. Configure required reviewers and
   restrict deployment tags to `v*`. Approve only reviewed tagged code: signing
   credentials must never be exposed to an untrusted tag or pull request.
3. Reuse your release signing key, or create one **once**, outside the repository.
   Back up both the keystore and its password securely; Android updates require
   the same signing key. For a new key, with a JDK installed:

   ```sh
   mkdir -p "$HOME/.config/thepeer.stream"
   chmod 700 "$HOME/.config/thepeer.stream"
   keytool -genkeypair -storetype PKCS12 \
     -keystore "$HOME/.config/thepeer.stream/release.p12" \
     -alias release -keyalg RSA -keysize 3072 -validity 10000 \
     -dname 'CN=thepeer.stream'
   ```

   Enter the password interactively. Do not pass passwords in shell arguments,
   commit the key, or regenerate it for each release. A local Android debug key
   is not the release key.
4. Set these four **environment secrets**, not workflow inputs:

   ```sh
   python3 -c 'import base64,pathlib,sys; sys.stdout.write(base64.b64encode((pathlib.Path.home()/".config/thepeer.stream/release.p12").read_bytes()).decode())' | gh secret set ANDROID_KEYSTORE_BASE64 --env release --repo benjaminpreiss/thepeer.stream
   gh secret set ANDROID_KEYSTORE_PASSWORD --env release --repo benjaminpreiss/thepeer.stream
   printf release | gh secret set ANDROID_KEY_ALIAS --env release --repo benjaminpreiss/thepeer.stream
   gh secret set ANDROID_KEY_PASSWORD --env release --repo benjaminpreiss/thepeer.stream
   ```

   The password commands prompt for hidden input. For the PKCS12 key above, use
   the same password for the keystore and key secrets. For an existing key, use
   its actual alias/passwords instead. The workflow decodes the keystore only
   into a private runner temporary directory, signs via password environment
   references, verifies the signature/alignment and deletes the temporary files.

## Create a draft

Commit/review the complete source, workflow, packaging scripts and `licenses/`
notices first. Then, from that clean reviewed commit:

```sh
git tag -a v0.1.0 -m 'thepeer.stream v0.1.0'
git push origin v0.1.0
gh workflow run release.yaml --repo benjaminpreiss/thepeer.stream --ref v0.1.0 -f android_version_code=7
```

Choose your actual version tag and a version code greater than every previously
published Android code; `7` is only the first example. The APK version name comes
from the tag without `v`. Tags with a suffix, such as `v0.1.0-rc.1`, create prerelease
drafts. Branch dispatches, malformed versions and invalid version codes are rejected.
The workflow verifies the tag still points at the built commit before creating the draft.
Do not move a released tag; make a new version.

The Actions UI can also run **Draft release**, selecting the tag and entering the
version code. Approve protected-environment jobs when prompted. Signing and draft
creation use the `release` environment; only the final job has `contents: write`.
An existing release is not overwritten. If asset upload fails, remove only the
incomplete **draft**, retain the tag, and rerun; never delete a published release
just to replace its assets.

## Assets and publication gate

- `thepeer.stream-TAG-linux-x86_64.tar.gz`: glibc 2.35+ (Ubuntu 22.04 baseline).
- `thepeer.stream-TAG-macos-arm64.tar.gz`: Apple silicon, macOS 15+.
- `thepeer.stream-TAG-macos-x86_64.tar.gz`: Intel, macOS 15+.
- `thepeer.stream-TAG-android-arm64.apk`: release-signed, Android 8.0/API 26+.
- `thepeer.stream-TAG-android-arm64-licenses.tar.gz`.
- `SHA256SUMS`: hashes of all five assets.

Helper archives contain `thepeer`, build metadata, the project license and
collected dependency notices. Rust collection includes a dependency superset,
including build/dev dependencies. Missing Rust license texts stop packaging;
reviewed version-specific supplements live in [licenses](../licenses/README.md).
Android bundles include resolved POMs and embedded AAR/JAR notices, not just a list
of dependency names. Review them and supplement missing texts/NOTICE obligations
before publication. Collection is not automatic legal certification.

Before publishing, verify checksums, inspect the bundle for unintended/private
contents, run the helper on each supported OS/architecture and repeat physical
Wi-Fi Direct, cellular Relay (with an authorized participant) and Stop tests on
the signed APK. Verify that an upgrade preserves the signing identity and uses a
higher version code. macOS binaries are not notarized; checksums are not signatures
or protection against a compromised publisher. The GitHub-hosted build matrix and
signing job must be validated by an actual operator-triggered run before claiming
release support. See [release readiness](RELEASE-READINESS.md).

Only then publish the draft manually in GitHub Releases.

## Installer behavior

The README's `curl … | sh` command uses `releases/latest`, which excludes drafts
and prereleases. Until a normal release is published it intentionally fails.
Install a specific published prerelease/version with:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/benjaminpreiss/thepeer.stream/main/scripts/install.sh | THEPEER_VERSION=v0.1.0-rc.1 sh
```

The script selects OS/architecture, verifies `SHA256SUMS`, extracts only the named
helper (not arbitrary archive paths), keeps the archive with its notices, and
replaces the binary without running it. Default prefix: `~/.local`; override with
`THEPEER_PREFIX` if needed. It does not use sudo or edit shell startup files.
Review the script before piping it to a shell. To remove the default installation,
delete `~/.local/bin/thepeer` and `~/.local/share/thepeer.stream/`.
