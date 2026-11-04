# Releases and compatibility

Cargo.toml is the single source of the application version. The first public
version is `0.1.0-alpha.1`, tagged `v0.1.0-alpha.1` and marked as a GitHub
prerelease. Releases currently contain source archives only. Cargo.lock is
committed; `publish = false` prevents accidental crates.io publication.

SemVer applies to documented user commands and behavior. Until 1.0, a minor
version may introduce incompatible changes; patch versions fix defects within
the current minor line. Alpha prereleases may change behavior between alpha
numbers. Rust modules are private implementation details. Profiling flags,
benchmark output schemas and synthetic dataset IDs are unstable diagnostics.
There is no persistent public file format yet.

Before a release: update Cargo.toml and Cargo.lock, update the changelog and
known limitations, run format/lint/tests and a clean release build, and review
third-party notices after dependency changes. Tag the reviewed commit, wait
for macOS CI, then create a GitHub prerelease using the changelog entry.
Never move a published tag; publish a new version for a correction.

`scripts/package-macos.sh` produces a local, ad-hoc signed development bundle.
Its numeric Apple version is derived from Cargo's version core; the manifest
retains the complete prerelease version. It is not a notarized distribution.
Public application binaries will require a separate signing, notarization,
license/notice and device-validation process before they are offered.
