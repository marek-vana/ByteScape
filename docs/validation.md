# Validation

Run source checks on macOS:

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
cargo build --locked --release
```

For integration measurements, use Python 3.11+ and a logged-in macOS desktop.
The fixture generator creates a new directory, never overwrites an existing
tree, and requires sufficient free space. This example creates 100,000 files
and a sparse file with a 512 MiB logical length:

```sh
python3 scripts/create-validation-fixture.py --files 100000 --manifest target/fixture-manifest.json
python3 scripts/validate-milestone.py --manifest target/fixture-manifest.json
python3 scripts/check-frame-budget.py
```

Reports and frame CSV files go under `target/validation-reports/`. They can
contain absolute paths; review and redact them before sharing. The runner
compares scan totals to independent Python lstat metadata and measures fast/
slow consumers, cancellation, a real empty tree, real scans and synthetic
million-item UI separately. Do not run builds or tests during measurements.
The frame budget checker returns nonzero if mean or p95 exceeds 1000/60 ms.
UI frame-start intervals are not physical presented-frame or GPU timing proof.

Diagnostic examples (paths must be explicit):

```sh
target/release/bytescape --scan-benchmark /path/to/fixture
target/release/bytescape --scan /path/to/fixture --profile
target/release/bytescape --million --profile
```

`BYTESCAPE_PROFILE_TO` selects the profile JSON destination;
`BYTESCAPE_PROFILE_FRAMES_TO` optionally selects raw frame CSV output.
`--poll-ms 3000` and `--cancel-after-ms 100` apply to `--scan-benchmark`.
`--profile-cancel-after-ms 100` exercises UI cancellation.
These diagnostic schemas and flags are unstable during alpha development.

Human keyboard/VoiceOver use, cloud providers, external devices and visual
acceptance require separate checks; see [limitations](limitations.md).
