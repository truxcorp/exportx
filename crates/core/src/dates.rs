//! Capture dates stored in photos and videos (ADR 0002).

use std::path::Path;

use chrono::{Datelike, Local, NaiveDateTime};
use nom_exif::{ExifDateTime, ExifTag, Metadata, TrackInfoTag};

/// The capture date stored in the file, in local time: EXIF `DateTimeOriginal` (else
/// `CreateDate`) for photos, read as camera-local time; QuickTime/MP4 creation time for
/// videos, stored in UTC and converted to the computer's time zone (dates before 1970 are
/// unset fields). `None` when the file has no readable date.
pub fn capture_date(path: &Path) -> Option<NaiveDateTime> {
    capture_date_with(path, |p| nom_exif::read_metadata(p))
}

/// `capture_date` with the metadata reader given: the seam tests use to stand in for the
/// library. A panic while reading (a parser bug on an odd file) counts as no date, so the
/// file falls back to its file time and the scan goes on. This needs `panic = "unwind"`
/// (Rust's default): never set `panic = "abort"` in a profile of the CLI or the app.
pub fn capture_date_with(
    path: &Path,
    read: impl FnOnce(&Path) -> nom_exif::Result<Metadata>,
) -> Option<NaiveDateTime> {
    let metadata = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read(path)));
    match metadata.ok()?.ok()? {
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
