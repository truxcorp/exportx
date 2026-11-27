//! Capture dates stored in photos and videos (ADR 0002).

use std::path::Path;

use chrono::{Datelike, Local, NaiveDateTime};
use nom_exif::{ExifDateTime, ExifTag, Metadata, TrackInfoTag};

/// Lower-case extensions whose capture date is read, each checked on a real sample
/// (`crates/cli/tests/fixtures`). Other files are dated by their file time.
pub const DATED_EXTS: &[&str] = &[
    "jpg", "heic", "cr2", "cr3", "nef", "arw", "dng", "mov", "mp4",
];

/// The capture date stored in the file, in local time: EXIF `DateTimeOriginal` (else
/// `CreateDate`) for photos, read as camera-local time; QuickTime/MP4 creation time for
/// videos, stored in UTC and converted to the computer's time zone (dates before 1970 are
/// unset fields). `None` when the file has no readable date.
pub fn capture_date(path: &Path) -> Option<NaiveDateTime> {
    capture_date_with(path, |p| nom_exif::read_metadata(p))
}

/// What reading a file's capture date gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureDate {
    Stored(NaiveDateTime),
    /// The file holds no capture date (or none this library reads).
    Missing,
    /// The metadata library crashed on this file: worth telling the user.
    Unreadable,
}

/// `capture_date_with`, telling a missing date from a crash of the library. The crash is
/// caught (this needs `panic = "unwind"`, Rust's default: never set `panic = "abort"` in a
/// profile of the CLI or the app) and prints nothing: the caller reports the file.
pub fn read_capture_date_with(
    path: &Path,
    read: impl FnOnce(&Path) -> nom_exif::Result<Metadata>,
) -> CaptureDate {
    install_quiet_panic_hook();
    QUIET.with(|q| q.set(true));
    let metadata = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read(path)));
    QUIET.with(|q| q.set(false));
    match metadata {
        Err(_) => CaptureDate::Unreadable,
        Ok(Err(_)) => CaptureDate::Missing,
        Ok(Ok(m)) => date_in(m).map_or(CaptureDate::Missing, CaptureDate::Stored),
    }
}

/// `capture_date` with the metadata reader given: the seam tests use to stand in for the
/// library. A crash while reading counts as no date, so the file falls back to its file
/// time and the scan goes on.
pub fn capture_date_with(
    path: &Path,
    read: impl FnOnce(&Path) -> nom_exif::Result<Metadata>,
) -> Option<NaiveDateTime> {
    match read_capture_date_with(path, read) {
        CaptureDate::Stored(d) => Some(d),
        CaptureDate::Missing | CaptureDate::Unreadable => None,
    }
}

thread_local! {
    /// Set while this thread reads metadata under the guard: its panics print nothing.
    static QUIET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Wrap the panic hook once: silent for guarded reads, unchanged for everything else.
fn install_quiet_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !QUIET.with(|q| q.get()) {
                previous(info);
            }
        }));
    });
}

fn date_in(metadata: Metadata) -> Option<NaiveDateTime> {
    match metadata {
        Metadata::Exif(exif) => [ExifTag::DateTimeOriginal, ExifTag::CreateDate]
            .into_iter()
            .find_map(|tag| exif.get(tag)?.as_datetime())
            .map(ExifDateTime::into_naive),
        Metadata::Track(track) => {
            let utc = match track.get(TrackInfoTag::CreateDate)?.as_datetime()? {
                ExifDateTime::Aware(t) => t.to_utc(),
                ExifDateTime::Naive(t) => t.and_utc(),
            };
            // QuickTime counts from 1904: tools that leave the field at 0 give 1904-01-01.
            (utc.year() >= 1970).then(|| utc.with_timezone(&Local).naive_local())
        }
    }
}
