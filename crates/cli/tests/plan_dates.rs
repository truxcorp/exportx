//! Behaviours 2-5: which date names the folders.
mod common;
use common::*;

#[test]
fn a_date_entered_wins_over_the_files_dates() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(&d.src, "2024-03-07 09:00", &["b.jpg"]);

    let out = plan(&d, &["--shoot", "s", "--date", "2019-07-14"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> 2019/07/14/s/p/o/jpg/txo_0001.jpg",
            "b.jpg -> 2019/07/14/s/p/o/jpg/txo_0002.jpg",
        ]
    );
}

/// The day folder `plan` picks for a single fixture file whose file time is 2000-01-01.
fn day_of(name: &str, tz: &str) -> String {
    let d = Dirs::new();
    fixture(&d.src, name, "2000-01-01 12:00");
    let out = exportpro_in_tz(tz, &plan_args(&d, &["--shoot", "s"]));
    assert_ok(&out);
    let line = moves(&out).pop().unwrap();
    line.split(" -> ").nth(1).unwrap()[..10].to_owned()
}

#[test]
fn without_a_date_entered_the_capture_date_stored_in_the_file_is_used() {
    let cases = [
        ("photo.jpg", "2021/06/15"),
        ("photo.heic", "2022/01/02"),
        ("photo.cr2", "2019/11/30"),
        ("photo.nef", "2019/12/01"),
        ("photo.arw", "2019/12/02"),
        ("photo.dng", "2019/12/03"),
        ("camera.cr3", "2025/09/06"), // EXIF, camera-local; its QuickTime date is 09-07 UTC
        ("clip.mp4", "2026/04/26"),
        ("clip.mov", "2025/02/24"),
    ];
    let got: Vec<(&str, String)> = cases.iter().map(|(f, _)| (*f, day_of(f, "UTC"))).collect();
    let want: Vec<(&str, String)> = cases.iter().map(|(f, d)| (*f, d.to_string())).collect();
    assert_eq!(got, want);
}

#[test]
fn video_dates_are_utc_and_shown_in_the_computers_time_zone() {
    // clip.mov: 2025-02-24 10:11:57 UTC is 2025-02-25 00:11:57 at UTC+14.
    assert_eq!(day_of("clip.mov", "Pacific/Kiritimati"), "2025/02/25");
}

#[test]
fn exif_dates_are_camera_local_and_never_shifted() {
    // photo.jpg: 2021-06-15 23:50:00 stays on the 15th whatever the computer's zone.
    assert_eq!(day_of("photo.jpg", "Pacific/Kiritimati"), "2021/06/15");
}

#[test]
fn a_file_without_a_stored_date_uses_its_file_time() {
    let d = Dirs::new();
    mk(&d.src, "2023-03-03 10:00", &["plain.jpg"]);

    let out = plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        ["plain.jpg -> 2023/03/03/s/p/o/jpg/txo_0001.jpg"]
    );
}

fn three_files_on_two_days(d: &Dirs) {
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(&d.src, "2024-03-05 18:00", &["b.CR2", "b.JPG"]);
    mk(&d.src, "2024-03-07 08:00", &["c.jpg"]);
}

#[test]
fn files_on_several_days_list_each_day_and_ask_for_a_choice() {
    let d = Dirs::new();
    three_files_on_two_days(&d);

    let out = plan(&d, &["--shoot", "s"]);

    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert_eq!(moves(&out), Vec::<String>::new());
    assert_eq!(
        stdout(&out).lines().collect::<Vec<_>>(),
        ["2024-03-05: 3 files", "2024-03-07: 1 file"]
    );
    assert!(stderr(&out).contains("--split"), "stderr: {}", stderr(&out));
}

#[test]
fn one_day_chosen_puts_every_file_in_that_days_folder() {
    let d = Dirs::new();
    three_files_on_two_days(&d);

    let out = plan(&d, &["--shoot", "s", "--date", "2024-03-05"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> 2024/03/05/s/p/o/jpg/txo_0001.jpg",
            "b.CR2 -> 2024/03/05/s/p/o/cr2/txo_0002.cr2",
            "b.JPG -> 2024/03/05/s/p/o/jpg/txo_0002.jpg",
            "c.jpg -> 2024/03/05/s/p/o/jpg/txo_0003.jpg",
        ]
    );
}

#[test]
fn each_day_chosen_splits_files_by_their_own_day() {
    let d = Dirs::new();
    three_files_on_two_days(&d);

    let out = plan(&d, &["--shoot", "s", "--split"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> 2024/03/05/s/p/o/jpg/txo_0001.jpg",
            "b.CR2 -> 2024/03/05/s/p/o/cr2/txo_0002.cr2",
            "b.JPG -> 2024/03/05/s/p/o/jpg/txo_0002.jpg",
            "c.jpg -> 2024/03/07/s/p/o/jpg/txo_0001.jpg",
        ]
    );
}

#[test]
fn a_group_spanning_midnight_stays_together_on_its_first_day() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 23:59", &["n.CR2"]);
    mk(&d.src, "2024-03-06 00:01", &["n.JPG"]);

    let out = plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "n.CR2 -> 2024/03/05/s/p/o/cr2/txo_0001.cr2",
            "n.JPG -> 2024/03/05/s/p/o/jpg/txo_0001.jpg",
        ]
    );
}

#[test]
fn no_date_drops_the_date_part_of_the_tree() {
    let d = Dirs::new();
    three_files_on_two_days(&d);

    let out = plan(&d, &["--shoot", "s", "--no-date"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> s/p/o/jpg/txo_0001.jpg",
            "b.CR2 -> s/p/o/cr2/txo_0002.cr2",
            "b.JPG -> s/p/o/jpg/txo_0002.jpg",
            "c.jpg -> s/p/o/jpg/txo_0003.jpg",
        ]
    );
}

/// Zero the creation and modification times of every mvhd/tkhd/mdhd box, as tools that
/// re-mux video often leave them (QuickTime's 0 is 1904-01-01).
fn zero_quicktime_dates(path: &std::path::Path) {
    let mut b = std::fs::read(path).unwrap();
    for name in [b"mvhd", b"tkhd", b"mdhd"] {
        let mut i = 0;
        while let Some(pos) = b[i..].windows(4).position(|w| w == name) {
            let at = i + pos + 4; // version byte
            let len = if b[at] == 1 { 16 } else { 8 };
            b[at + 4..at + 4 + len].fill(0);
            i = at;
        }
    }
    std::fs::write(path, b).unwrap();
}

#[test]
fn a_video_with_a_zero_creation_time_uses_its_file_time() {
    let d = Dirs::new();
    fixture(&d.src, "clip.mp4", "2000-01-01 12:00");
    zero_quicktime_dates(&d.src.join("clip.mp4"));
    set_mtime(&d.src.join("clip.mp4"), epoch("2023-08-09 10:00"));

    let out = plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        ["clip.mp4 -> 2023/08/09/s/v/o/mp4/txo_0001.mp4"]
    );
}
