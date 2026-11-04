# Changelog

## 0.1.0-alpha.2 — 2026-11-04

- Fix an accessibility regression test that incorrectly rejected the permitted
  five-second progress announcement on a slower hosted runner. The test now
  checks the actual throttle interval and still verifies keyboard cancellation.
- Move hosted source checks to macOS 15 Intel runners after arm64 capacity
  delays. Native UI validation remains separate.
- Application behavior is unchanged. The alpha.1 source tag is preserved.


## 0.1.0-alpha.1 — 2026-11-04

First public, source-only alpha.

- Native GPU map, rings, virtualized file list and inspector.
- Metadata scans with cancellation, partial results, logical/allocated sizes,
  deterministic hard-link accounting, symlink boundaries and macOS packages.
- Search, Czech/English UI, keyboard navigation and reduced motion.
- Build instructions, MIT licensing, font notices and macOS CI.

No downloadable application is included. Performance, accessibility and real
cloud/external-volume acceptance remain open; see docs/limitations.md.
