//! Capture dates stored in photos and videos (ADR 0002).

use std::path::Path;

use chrono::{Datelike, Local, NaiveDateTime};
use nom_exif::{ExifDateTime, ExifTag, Metadata, TrackInfoTag};

/// The capture date stored in the file, in local time: EXIF `DateTimeOriginal` (else
/// `CreateDate`) for photos, read as camera-local time; QuickTime/MP4 creation time for
/// videos, stored in UTC and converted to the computer's time zone (dates before 1970 are
/// unset fields). `None` when the file has no readable date.
pub fn capture_date(path: &Path) -> Option<NaiveDateTime> {
    match nom_exif::read_metadata(path).ok()? {
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
