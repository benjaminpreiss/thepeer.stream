# License and third-party notices

thepeer.stream is licensed under the [MIT License](LICENSE), copyright 2026
Benjamin Preiss. Third-party code, dependencies and build tools retain their own
licenses and notices; the project license does not override them.

## Source included in this repository

- **Saorsa transport patches:** the [reviewed patches](patches/saorsa-transport/README.md)
  contain upstream source context used under its MIT option. Source headers retain
  the Saorsa Labs attribution. The verbatim
  [upstream MIT license](patches/saorsa-transport/UPSTREAM-LICENSE-MIT), including
  the quinn Developers notice, accompanies the patches.
- **Candidate planner:** [route_plan.rs](crates/tunnel-core/src/route_plan.rs)
  adapts Saorsa-core address classification and contextual planning under its MIT
  option. Its source attribution and verbatim
  [MIT notice](crates/tunnel-core/src/route-plan-LICENSE-MIT), copyright the
  Autonomi Foundation, are retained.
- **Gradle wrapper:** the Android wrapper scripts retain their original copyright
  and Apache-2.0 headers. `gradle-wrapper.jar` contains the license in
  `META-INF/LICENSE`. These are third-party build tools, not project-authored code.

## Binary distributions

APK/helper releases must include the licenses and copyright/NOTICE texts required
by the dependencies actually shipped, including Rust/native and Android/Gradle
runtime dependencies. Cargo metadata alone is not a complete binary notice bundle.
Do not distribute ignored caches, local credentials or retired build artifacts.

The release packager collects resolved source notices and uses reviewed
[version-specific supplements](licenses/README.md) where published crates omit
license texts. Android reports include resolved POMs and embedded AAR/JAR notices.
The complete distribution notice review remains a [release gate](docs/RELEASE-READINESS.md).
