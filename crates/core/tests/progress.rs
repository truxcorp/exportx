//! The copy's progress, by bytes and files (slice 1 of the move & progress plan).

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use exportpro_core::Lang;
use exportpro_core::progress::{Meter, Progress, Report};
use exportpro_core::{DateChoice, Layout, Plan, Request};

fn request(dir: &Path) -> Request {
    Request {
        dest: dir.join("photos"),
        shoot: "s".into(),
        prefix: "txo".into(),
        date: DateChoice::NoDate,
        layout: Layout::default(),
    }
}

fn plan(src: &Path, req: &Request) -> Plan {
    exportpro_core::plan(&exportpro_core::scan(src).unwrap(), req).unwrap()
}

#[test]
fn progress_counts_bytes_and_files_of_the_files_to_copy_only() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "already copied").unwrap();
    let req = request(dir.path());
    exportpro_core::apply(&plan(&src, &req), &req);
    fs::write(src.join("b.mov"), vec![7u8; 3_000_000]).unwrap();
    let mut seen: Vec<Progress> = Vec::new();

    exportpro_core::apply_with_progress(&plan(&src, &req), &req, |p| seen.push(p));

    let last = *seen.last().expect("some progress");
    assert_eq!(
        last,
        Progress {
            files_done: 1,
            files_total: 1,
            bytes_done: 3_000_000,
            bytes_total: 3_000_000,
        }
    );
    assert!(
        seen.iter()
            .all(|p| p.bytes_total == 3_000_000 && p.files_total == 1)
    );
    assert!(seen.windows(2).all(|w| w[0].bytes_done <= w[1].bytes_done));
}

#[test]
fn progress_moves_within_a_large_file() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("big.mov"), vec![7u8; 5_000_000]).unwrap();
    let req = request(dir.path());
    let mut seen: Vec<Progress> = Vec::new();

    exportpro_core::apply_with_progress(&plan(&src, &req), &req, |p| seen.push(p));

    assert!(
        seen.iter()
            .any(|p| p.files_done == 0 && p.bytes_done > 0 && p.bytes_done < 5_000_000),
        "no progress inside the file: {seen:?}"
    );
}

fn at(bytes_done: u64, files_done: usize) -> Progress {
    Progress {
        files_done,
        files_total: 2,
        bytes_done,
        bytes_total: 1_000_000_000,
    }
}

#[test]
fn the_meter_reports_at_most_ten_times_a_second_but_always_the_end() {
    let t0 = Instant::now();
    let ms = |n| t0 + Duration::from_millis(n);
    let mut meter = Meter::new(t0);

    let sent: Vec<bool> = [
        (10, at(1, 0)),
        (50, at(2, 0)),
        (99, at(3, 1)),
        (120, at(4, 1)),
        (130, at(1_000_000_000, 2)),
    ]
    .into_iter()
    .map(|(t, p)| meter.update(ms(t), p).is_some())
    .collect();

    assert_eq!(sent, [true, false, false, true, true]);
}

#[test]
fn the_time_left_shows_after_three_seconds_from_the_last_five_seconds_speed() {
    let t0 = Instant::now();
    let mut meter = Meter::new(t0);
    let mut left = |ms: u64, bytes: u64| {
        let report = meter.update(t0 + Duration::from_millis(ms), at(bytes, 0));
        report.expect("a report is due").secs_left
    };

    assert_eq!(left(1_000, 10_000_000), None);
    assert_eq!(left(2_900, 29_000_000), None);
    // 10 MB/s so far, 970 MB left.
    assert_eq!(left(3_000, 30_000_000), Some(97));
    // Then 50 MB/s for 6 s: the first seconds no longer count.
    let mut bytes = 30_000_000;
    for tenth in 31..=90 {
        bytes += 5_000_000;
        left(tenth * 100, bytes);
    }
    // 665 MB left at 50 MB/s.
    assert_eq!(left(9_100, bytes + 5_000_000), Some(13));
}

fn report(
    files_done: usize,
    files_total: usize,
    bytes: (u64, u64),
    secs_left: Option<u64>,
) -> Report {
    Report {
        progress: Progress {
            files_done,
            files_total,
            bytes_done: bytes.0,
            bytes_total: bytes.1,
        },
        secs_left,
    }
}

#[test]
fn the_progress_line_tells_files_sizes_and_time_left_in_both_languages() {
    let r = report(312, 1204, (18_200_000_000, 64_000_000_000), Some(331));
    assert_eq!(
        Lang::En.progress(&r),
        "312 / 1,204 files · 18.2 GB / 64.0 GB · about 6 min left"
    );
    assert_eq!(
        Lang::Fr.progress(&r),
        "312 / 1\u{a0}204 fichiers · 18,2 Go / 64,0 Go · environ 6 min restantes"
    );
    let r = report(0, 1, (0, 5_000), None);
    assert_eq!(Lang::En.progress(&r), "0 / 1 file · 0 B / 5.0 kB");
    assert_eq!(Lang::Fr.progress(&r), "0 / 1 fichier · 0 o / 5,0 ko");
    let r = report(2, 9, (1, 9), Some(42));
    assert!(
        Lang::En
            .progress(&r)
            .ends_with(" · less than a minute left")
    );
    assert!(
        Lang::Fr
            .progress(&r)
            .ends_with(" · moins d'une minute restante")
    );
    let r = report(2, 9, (1, 9), Some(3_900));
    assert!(Lang::En.progress(&r).ends_with(" · about 1 h 05 left"));
    assert!(
        Lang::Fr
            .progress(&r)
            .ends_with(" · environ 1 h 05 restantes")
    );
}

#[test]
fn a_finished_copy_tells_no_time_left() {
    let t0 = Instant::now();
    let mut meter = Meter::new(t0);
    let end = Progress {
        files_done: 2,
        files_total: 2,
        bytes_done: 1_000,
        bytes_total: 1_000,
    };

    let report = meter.update(t0 + Duration::from_secs(40), end).unwrap();

    assert_eq!(report.secs_left, None);
}
