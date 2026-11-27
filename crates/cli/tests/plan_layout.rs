//! Configurable tree (ADR 0004): presets, custom templates and their validation.
mod common;
use common::*;

fn one_photo_one_video() -> Dirs {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(&d.src, "2024-03-05 09:05", &["b.mov"]);
    d
}

#[test]
fn the_lisible_preset_uses_readable_folder_names() {
    let d = one_photo_one_video();

    let out = plan(&d, &["--shoot", "s", "--preset", "lisible"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> 2024/03/05/s/photo/originaux/jpg/txo_0001.jpg",
            "b.mov -> 2024/03/05/s/video/originaux/mov/txo_0002.mov",
        ]
    );
}

#[test]
fn the_classique_preset_is_the_default() {
    let d = one_photo_one_video();

    let with = plan(&d, &["--shoot", "s", "--preset", "classique"]);
    let without = plan(&d, &["--shoot", "s"]);

    assert_ok(&with);
    assert_eq!(moves(&with), moves(&without));
}

#[test]
fn custom_templates_reshape_the_tree() {
    let d = one_photo_one_video();

    let out = plan(
        &d,
        &[
            "--shoot",
            "s",
            "--shoot-path",
            "{shoot}/{yyyy}-{mm}-{dd}",
            "--file-path",
            "{kind}/{prefix}-{n:2}.{ext}",
        ],
    );

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> s/2024-03-05/p/txo-01.jpg",
            "b.mov -> s/2024-03-05/v/txo-02.mov",
        ]
    );
}

#[test]
fn custom_numbers_continue_with_their_own_file_name_template() {
    let d = one_photo_one_video();
    mk(
        &d.dest.join("s/2024-03-05/p"),
        "2024-03-05 08:00",
        &["txo-5.jpg"],
    );

    let out = plan(
        &d,
        &[
            "--shoot",
            "s",
            "--shoot-path",
            "{shoot}/{yyyy}-{mm}-{dd}",
            "--file-path",
            "{kind}/{prefix}-{n:2}.{ext}",
        ],
    );

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> s/2024-03-05/p/txo-06.jpg",
            "b.mov -> s/2024-03-05/v/txo-07.mov",
        ]
    );
}

#[test]
fn invalid_templates_are_refused_before_any_plan() {
    let cases: &[(&str, &str, &str)] = &[
        ("--file-path", "{kind}/{bogus}.{ext}", "{bogus}"),
        ("--file-path", "{kind}/{prefix}_{n.{ext}", "not closed"),
        ("--file-path", "{kind}/{prefix}.{ext}", "{n}"),
        ("--file-path", "{kind}/{prefix}_{n}", "{ext}"),
        ("--file-path", "{n}/{prefix}.{ext}", "{n}"),
        ("--file-path", "/{kind}/{prefix}_{n}.{ext}", "absolute"),
        ("--file-path", "{kind}/../{prefix}_{n}.{ext}", ".."),
        ("--shoot-path", "/{shoot}", "absolute"),
        ("--shoot-path", "{shoot}/..", ".."),
    ];
    let mut failures = Vec::new();
    for (flag, template, expected) in cases {
        let d = one_photo_one_video();
        let out = plan(&d, &["--shoot", "s", flag, template]);
        let err = stderr(&out);
        if out.status.code() != Some(1) || !moves(&out).is_empty() || !err.contains(expected) {
            failures.push(format!(
                "{flag} {template}: exit {:?}, stderr {err:?}",
                out.status.code()
            ));
        }
    }
    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn an_unknown_preset_is_refused() {
    let d = one_photo_one_video();

    let out = plan(&d, &["--shoot", "s", "--preset", "fancy"]);

    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("fancy"), "stderr: {}", stderr(&out));
}

// Review of PR #11: reproductions.

#[test]
fn a_shoot_path_needs_shoot_and_no_per_file_tokens() {
    let cases: &[(&str, &str)] = &[
        ("{shoot}/{kind}", "{kind}"),
        ("{shoot}/{ext}", "{ext}"),
        ("{shoot}/{n}", "{n}"),
        ("{yyyy}/{mm}", "{shoot}"),
    ];
    let mut failures = Vec::new();
    for (template, expected) in cases {
        let d = one_photo_one_video();
        let out = plan(&d, &["--shoot", "s", "--shoot-path", template]);
        let err = stderr(&out);
        if out.status.code() != Some(1) || !err.contains(expected) {
            failures.push(format!(
                "{template}: exit {:?}, stderr {err:?}",
                out.status.code()
            ));
        }
    }
    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn split_days_sharing_one_folder_share_one_numbering() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(&d.src, "2024-03-06 09:00", &["b.jpg"]);

    let out = plan(&d, &["--shoot", "s", "--split", "--shoot-path", "{shoot}"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> s/p/o/jpg/txo_0001.jpg",
            "b.jpg -> s/p/o/jpg/txo_0002.jpg"
        ]
    );
}

#[test]
fn no_date_drops_the_separators_around_date_tokens() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let dashed = plan(
        &d,
        &[
            "--shoot",
            "s",
            "--no-date",
            "--shoot-path",
            "{shoot}/{yyyy}-{mm}-{dd}",
        ],
    );
    let joined = plan(
        &d,
        &[
            "--shoot",
            "s",
            "--no-date",
            "--shoot-path",
            "{shoot}_{yyyy}",
        ],
    );

    assert_ok(&dashed);
    assert_ok(&joined);
    assert_eq!(moves(&dashed), ["a.jpg -> s/p/o/jpg/txo_0001.jpg"]);
    assert_eq!(moves(&joined), ["a.jpg -> s/p/o/jpg/txo_0001.jpg"]);
}

#[test]
fn a_huge_number_width_is_a_template_error() {
    let d = one_photo_one_video();

    let out = plan(
        &d,
        &[
            "--shoot",
            "s",
            "--file-path",
            "{kind}/{prefix}_{n:99999999999}.{ext}",
        ],
    );

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("invalid tree"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn identical_destinations_are_not_called_a_case_difference() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.JPG", "a.jpg"]);

    let out = plan(&d, &["--shoot", "s"]);

    assert_eq!(out.status.code(), Some(1));
    assert!(
        !stderr(&out).contains("by case"),
        "stderr: {}",
        stderr(&out)
    );
}
