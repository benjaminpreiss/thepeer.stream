# Release notice supplements

The packager collects license/NOTICE files from the exact Cargo-resolved source
archives. Some published crates omit them. These version-specific supplements
come from primary sources, not guessed author/copyright templates. A dependency
upgrade must review its supplements again. Tests reject missing Rust notice texts.

The MIT texts below were retrieved verbatim from the source revisions
recorded in each published crate's `.cargo_vcs_info.json`:

| Package | Source text |
| --- | --- |
| asn1-rs-impl 0.2.0 | [LICENSE-MIT](https://github.com/rusticata/asn1-rs/blob/a20e5f7319c896737ad0f2557037817b91ad854f/LICENSE-MIT) |
| jni 0.22.4 | [LICENSE-MIT](https://github.com/jni-rs/jni-rs/blob/5ae9458a4ec44c5318f37ddc7569c1d4ae8a69e7/LICENSE-MIT) |
| jni-macros 0.22.4 | [LICENSE-MIT](https://github.com/jni-rs/jni-rs/blob/33045a124105c939d1e2cbdcb5a39e5d868ffa03/LICENSE-MIT) |
| jni-sys-macros 0.4.1 | [LICENSE-MIT](https://github.com/jni-rs/jni-sys/blob/64d77b7a5f119d7b55b4e2c169a4668067ff59e6/LICENSE-MIT) |
| rustls-post-quantum 0.2.4 | [LICENSE-MIT](https://github.com/rustls/rustls/blob/9198bbd1446ee1b4ef3fd6fcf27d421c40080c61/LICENSE-MIT) |
| rustls-platform-verifier-android 0.2.0 | [LICENSE-MIT](https://github.com/rustls/rustls-platform-verifier/blob/252e25161a91af476cbea620e29d277592d27ec7/LICENSE-MIT) |

saorsa-pqc 0.5.2 has no license file in its published crate or recorded source tree.
Its [manifest](https://github.com/saorsa-labs/saorsa-pqc/blob/b90282e5dc87a5643d0dcdb039559ea32cd6896f/Cargo.toml)
and [README](https://github.com/saorsa-labs/saorsa-pqc/blob/b90282e5dc87a5643d0dcdb039559ea32cd6896f/README.md)
explicitly offer MIT or Apache-2.0. We select Apache-2.0 and include the
[canonical full text](https://www.apache.org/licenses/LICENSE-2.0.txt), plus the
upstream author and source reference in `overrides/saorsa-pqc-0.5.2/NOTICE`.

Android packaging collects resolved dependency POMs and embedded AAR/JAR notices,
including nested JARs. The full Apache-2.0 text is also supplied by Gradle's wrapper.
Before publishing a draft, review actual Android license declarations and NOTICE
requirements. Add any missing reviewed texts under `licenses/android/` and rebuild.
The mere presence of a text file is not a legal-compliance certification.
