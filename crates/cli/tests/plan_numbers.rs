//! Behaviour 6: numbers continue after the highest one already in the shoot folder
//! (ported from tests/test_exportpro.sh, case 4).
mod common;
use common::*;

#[test]
fn numbers_continue_after_the_highest_in_the_shoot_folder_padded_or_not() {
    let d = Dirs::new();
    let shoot = d.dest.join("2021/05/05/expo");
    mk(&shoot.join("p/o/jpg"), "2021-05-05 10:00", &["x_0002.jpg"]);
    mk(&shoot.join("p/o/cr2"), "2021-05-05 10:00", &["x_7.cr2"]); // made by v0.2.0
    mk(
        &shoot.join("p/o/jpg"),
        "2021-05-05 10:00",
        &["x_abc.jpg", "y_0099.jpg", "x_0050"],
    );
    mk(&d.src, "2021-05-05 11:00", &["c.jpg", "d.jpg"]);

    let out = plan(&d, &["--shoot", "expo", "--prefix", "x"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "c.jpg -> 2021/05/05/expo/p/o/jpg/x_0008.jpg",
            "d.jpg -> 2021/05/05/expo/p/o/jpg/x_0009.jpg",
        ]
    );
}

#[test]
fn split_days_continue_their_own_folders_numbers() {
    let d = Dirs::new();
    mk(
        &d.dest.join("2024/03/05/s/p/o/jpg"),
        "2024-03-05 10:00",
        &["txo_0003.jpg"],
    );
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(&d.src, "2024-03-07 08:00", &["c.jpg"]);

    let out = plan(&d, &["--shoot", "s", "--split"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> 2024/03/05/s/p/o/jpg/txo_0004.jpg",
            "c.jpg -> 2024/03/07/s/p/o/jpg/txo_0001.jpg",
        ]
    );
}

#[test]
fn same_time_files_are_numbered_by_name() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["b.jpg", "a.jpg", "C.jpg"]);

    let out = plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> 2024/03/05/s/p/o/jpg/txo_0001.jpg",
            "b.jpg -> 2024/03/05/s/p/o/jpg/txo_0002.jpg",
            "C.jpg -> 2024/03/05/s/p/o/jpg/txo_0003.jpg",
        ]
    );
}
