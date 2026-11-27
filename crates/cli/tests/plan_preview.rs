//! Behaviour 8: the preview lists every original and its destination; the plan is
//! refused if any destination already exists.
mod common;
use common::*;

#[test]
fn the_preview_lists_every_original_and_its_destination() {
    let d = Dirs::new();
    mk(
        &d.src,
        "2024-03-05 09:00",
        &["IMG_1.CR2", "IMG_1.JPG", "MVI_2.MOV"],
    );

    let out = plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert_eq!(
        stdout(&out).lines().collect::<Vec<_>>(),
        [
            "IMG_1.CR2 -> 2024/03/05/s/p/o/cr2/txo_0001.cr2",
            "IMG_1.JPG -> 2024/03/05/s/p/o/jpg/txo_0001.jpg",
            "MVI_2.MOV -> 2024/03/05/s/v/o/mov/txo_0002.mov",
        ]
    );
}

#[test]
fn a_destination_that_already_exists_refuses_the_plan() {
    let d = Dirs::new();
    // Not a numbered file, so numbering starts at 1, but something is in the way.
    std::fs::create_dir_all(d.dest.join("2024/03/05/s/p/o/jpg/txo_0001.jpg")).unwrap();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let out = plan(&d, &["--shoot", "s"]);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert_eq!(moves(&out), Vec::<String>::new());
    assert!(
        stderr(&out).contains("2024/03/05/s/p/o/jpg/txo_0001.jpg")
            && stderr(&out).contains("exists"),
        "stderr: {}",
        stderr(&out)
    );
}
