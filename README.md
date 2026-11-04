# ByteScape

Native disk cartography for macOS, built with Rust, egui/eframe and wgpu/Metal.
Explore a folder through a GPU map, concentric rings, a file list and an inspector.

**0.1.0-alpha.1 is an experimental, source-only release.** Build it locally;
there is no downloadable signed or notarized application yet.
[Czech quick start](docs/README.cs.md) · [Known limitations](docs/limitations.md)

## Build and run

The verified environment is Apple Silicon (M1, 8 GiB), macOS 27.0.1 and
Rust 1.99.1. Other macOS versions and Intel Macs have not been validated.
Install Rust 1.99.1 or newer and the Xcode command line tools (`xcode-select --install`).
Python 3.11+ is needed only for validation scripts.

```sh
git clone https://github.com/marek-vana/ByteScape.git
cd ByteScape
cargo run --locked --release
```

The initial view uses labeled demonstration data. Press **⌘O** to choose a real
folder. Results stay in memory and are not saved between launches.

| Control | Action |
| --- | --- |
| ⌘O | Choose a folder and scan |
| ⌘. | Cancel; keep the partial result |
| ⌘R | Scan the same folder again |
| ⌘I | Inspector and problems |
| Tab / Shift+Tab | Move focus |
| Arrows / Enter | Select / enter a folder or aggregate |
| Backspace | Parent scope |

The UI includes Czech/English, text scaling, reduced motion, search, and logical
versus allocated size. Scans use filesystem metadata, do not read file contents,
do not follow symlinks, and stay on the selected filesystem. Hard links count
once; macOS packages can be explored. Unknown or inaccessible data is reported
as incomplete rather than zero. Allocated bytes are **not** reclaimable space.
There is no deletion, Finder integration, telemetry, or persistent scan history.
Real cloud-provider behavior still needs validation; see the limitations before
using the alpha with cloud folders.

## Development

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
cargo build --locked --release
```

[Validation and diagnostic commands](docs/validation.md) explain reproducible
fixtures and the limits of performance measurements. The macOS CI checks the
source build; human VoiceOver and visual acceptance remain separate checks.

## Contributions and versioning

Issues and pull requests are welcome on a best-effort basis, without response
or delivery deadlines. Read [CONTRIBUTING.md](CONTRIBUTING.md) and
[SECURITY.md](SECURITY.md). Versions follow [SemVer](https://semver.org/):
this is pre-1.0 software and minor versions may change documented behavior.
See [release policy](docs/releases.md) and [CHANGELOG.md](CHANGELOG.md).

## License and credits

Original project code is [MIT licensed](LICENSE). IBM Plex Mono and Saira
Condensed use the SIL Open Font License 1.1; their copyright and license files
are in `assets/fonts/`. Rust dependencies have their own licenses listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
The visual exploration was inspired by orchestra_os and cinematic disk maps;
third-party reference images and internal research are not distributed here.
