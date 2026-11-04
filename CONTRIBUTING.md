# Contributing

Issues and pull requests are welcome. Maintenance is best effort with no
response or delivery deadlines. Please be respectful and keep discussion
focused on the work. Report security problems using SECURITY.md.

For a bug, include your macOS version, architecture, Rust version, reproduction
steps and expected/actual behavior. Prefer a synthetic example. Remove private
paths, filenames and file contents from logs and screenshots before posting.
For substantial behavior or UI changes, open an issue to discuss scope first.

Build on macOS and run `cargo test --locked`, `cargo clippy --locked --all-targets
-- -D warnings`, and `cargo fmt --check`. Describe what you verified and any
remaining human or device checks. Do not include generated fixtures, build
outputs, personal filesystem reports, or unrelated formatting changes.

Contributions are under the project's MIT license. Preserve third-party
copyrights and license files; disclose any new assets or dependencies.
