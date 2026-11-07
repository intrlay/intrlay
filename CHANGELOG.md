# Changelog

## [0.1.2]

- Support installing the prebuilt CLI with `cargo binstall intrlay-cli`.
- Verify native archives through cargo-binstall before publication and avoid GNU
  sparse tar entries that binary installers can skip.
- Validate and publish Dart archives outside Git ignore rules, and request the
  publishing token after building the release tooling.
- Update installation instructions and document the Linux binary requirements.
- Fix shared README links on crates.io and refresh API and platform documentation.

## [0.1.1]

- Distribute prebuilt CLI binaries for Windows, Linux, and macOS on x64 and ARM64,
  with C++ headers and SHA-256 checksums in GitHub Releases.
- Publish runtime packages through GitHub Actions using trusted publishing.
- Verify clean release sources and include only current workspace crate archives
  in release artifacts.

## [0.1.0]

- Initial release.
