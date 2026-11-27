//! Behaviour 1: scanning the source folder (ported from tests/test_exportpro.sh, cases 1-3 and 7).
mod common;
use common::*;

#[test]
fn pairs_share_a_number_and_numbers_follow_time() {
    let d = Dirs::new();
    mk(
        &d.src,
        "2018-10-23 14:30",
        &["IMG_0002.CR2", "IMG_0002.JPG"],
    );
    mk(
        &d.src,
        "2018-10-23 14:10",
        &["IMG_0009.CR2", "IMG_0009.JPG"],
    ); // earlier, higher number
    mk(&d.src, "2018-10-23 14:20", &["IMG_0005.JPG"]);

    let out = plan(&d, &["--shoot", "mariage", "--prefix", "dupont"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "IMG_0009.CR2 -> 2018/10/23/mariage/p/o/cr2/dupont_0001.cr2",
            "IMG_0009.JPG -> 2018/10/23/mariage/p/o/jpg/dupont_0001.jpg",
            "IMG_0005.JPG -> 2018/10/23/mariage/p/o/jpg/dupont_0002.jpg",
            "IMG_0002.CR2 -> 2018/10/23/mariage/p/o/cr2/dupont_0003.cr2",
            "IMG_0002.JPG -> 2018/10/23/mariage/p/o/jpg/dupont_0003.jpg",
        ]
    );
}

#[test]
fn a_group_with_a_video_is_a_video_group() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["A.CR2", "A.JPG"]);
    mk(&d.src, "2024-03-05 09:05", &["MVI_1.MOV", "MVI_1.THM"]);
    mk(&d.src, "2024-03-05 09:10", &["clip.mp4", "notes.xmp"]);

    let out = plan(&d, &["--shoot", "concert", "--prefix", "txo"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "A.CR2 -> 2024/03/05/concert/p/o/cr2/txo_0001.cr2",
            "A.JPG -> 2024/03/05/concert/p/o/jpg/txo_0001.jpg",
            "MVI_1.MOV -> 2024/03/05/concert/v/o/mov/txo_0002.mov",
            "MVI_1.THM -> 2024/03/05/concert/v/o/thm/txo_0002.thm",
            "clip.mp4 -> 2024/03/05/concert/v/o/mp4/txo_0003.mp4",
            "notes.xmp -> 2024/03/05/concert/p/o/xmp/txo_0004.xmp",
        ]
    );
}

#[test]
fn hidden_files_files_without_extension_and_subfolders_are_skipped() {
    let d = Dirs::with_source_name("with space");
    mk(
        &d.src,
        "2020-01-01 12:00",
        &["my photo.JPG", ".DS_Store", "README", ".hidden.jpg"],
    );
    mk(&d.src.join("sub"), "2020-01-01 12:00", &["deep.jpg"]);

    let out = plan(&d, &["--shoot", "shoot one"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        ["my photo.JPG -> 2020/01/01/shoot one/p/o/jpg/txo_0001.jpg"]
    );
}

#[test]
fn an_empty_source_is_refused() {
    let d = Dirs::new();
    mk(&d.src, "2020-01-01 12:00", &[".DS_Store"]);

    let out = plan(&d, &["--shoot", "x"]);

    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("no files"),
        "stderr: {}",
        stderr(&out)
    );
}
