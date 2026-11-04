# Alpha limitations

The release is source-only, experimental, and tested on M1 / 8 GiB,
macOS 27.0.1 with Rust 1.99.1. No older OS or Intel UI compatibility is promised. Hosted macOS 15 Intel
build/test checks are separate from native UI acceptance.

- The strict 60 FPS UI interval target remains unmet. A synthetic million-file
  run measured mean 16.66775 ms and p95 17.375667 ms. These are frame-start
  intervals, not GPU timestamps or proof of presented frames.
- Human VoiceOver listening and final visual acceptance are pending. Automated
  accessibility inspection does not establish a complete accessible workflow.
- Real iCloud/File Provider no-download behavior is unverified. The scanner uses
  metadata and recognizes cloud states; this is not a provider-wide guarantee
  that scanning never triggers network traffic or materialization.
- Physical external-volume and disconnect scenarios are unverified.
- Native paths are preserved internally, but actual non-UTF-8 filename creation
  was rejected by the tested APFS environment; full real-filesystem coverage
  of such paths is incomplete.
- Permission errors, cancelled scans and unknown allocated sizes can produce
  partial results. Allocated size does not estimate reclaimable bytes.
- Scans, search results and navigation exist only in memory. No deletion,
  persistent scan history, or Finder integration is implemented.

The existing 32 automated tests check model, scan and UI behavior. They do not
replace the outstanding device, provider and human acceptance checks.
