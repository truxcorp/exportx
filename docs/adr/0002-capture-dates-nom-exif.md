# 0002. Read capture dates with nom-exif, file times as fallback
Date: 2026-11-26 · Status: Accepted
## Context
v0.2.0 dates files by their file times, which copies can reset. The shoot date and the
numbering order should come from the capture date stored in photos (EXIF, incl. raw
formats CR2, CR3, NEF, ARW, DNG, HEIC) and videos (QuickTime / MP4 creation time).
A date typed by the user overrides everything.
## Options
- A: `nom-exif` — pure Rust, reads EXIF from images and raw files and track metadata
  from MOV/MP4; one dependency.
- B: `kamadak-exif` + a separate MP4 parser — two libraries, gaps on some raw formats.
- C: bundle `exiftool` — most complete; needs Perl, heavier bundle, one process per run.
## Decision
A, behind a small `capture_date(path) -> Option<NaiveDateTime>` function in the core so
it can be swapped. Fallback: the earliest of the file's creation and modification times.
EXIF times are camera-local (no timezone); QuickTime times are UTC and are converted to
the computer's timezone.
## Consequences
Test fixtures need small real sample files (one per supported format) in
`crates/core/tests/fixtures/`. If a format turns out unsupported, it falls back to file
times and the plan says so.
