//! Behaviour 14: messages follow the system language (French or English), with a switch.
mod common;
use common::*;

fn several_days(d: &Dirs) {
    mk(&d.src, "2024-03-05 09:00", &["a.jpg", "b.jpg"]);
    mk(&d.src, "2024-03-07 09:00", &["c.jpg"]);
}

#[test]
fn a_french_system_gets_french_messages() {
    let d = Dirs::new();
    several_days(&d);

    let out = exportpro_with(
        &[("LANG", "fr_FR.UTF-8")],
        &plan_args(&d, &["--shoot", "s"]),
    );

    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        stdout(&out).lines().collect::<Vec<_>>(),
        ["2024-03-05 : 2 fichiers", "2024-03-07 : 1 fichier"]
    );
    assert!(
        stderr(&out).contains("plusieurs jours"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn lc_all_wins_over_lang() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &[".DS_Store"]);

    let out = exportpro_with(
        &[("LANG", "en_US.UTF-8"), ("LC_ALL", "fr_CA.UTF-8")],
        &plan_args(&d, &["--shoot", "s"]),
    );

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("aucun fichier"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn any_other_language_gets_english() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &[".DS_Store"]);

    let out = exportpro_with(
        &[("LANG", "de_DE.UTF-8")],
        &plan_args(&d, &["--shoot", "s"]),
    );

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("no files"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn the_lang_option_switches_language() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let out = exportpro_with(
        &[("LANG", "en_US.UTF-8")],
        &command_args("apply", &d, &["--shoot", "s", "--lang", "fr"]),
    );

    assert_ok(&out);
    assert!(
        stdout(&out).contains("1 fichier(s) copié(s) ; les originaux sont intacts"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn refusals_are_translated() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let out = exportpro_with(
        &[("LANG", "fr_FR.UTF-8")],
        &plan_args(&d, &["--shoot", "CON"]),
    );

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("nom invalide \"CON\" : il est réservé sous Windows"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn the_french_erase_summary_reads_right() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    assert_ok(&apply(&d, &["--shoot", "s"]));
    let shoot = d.dest.join("2024/03/05/s");
    let data = d.home.join(".local/share");

    let out = exportpro_with(
        &[
            ("LANG", "fr_FR.UTF-8"),
            ("HOME", d.home.to_str().unwrap()),
            ("XDG_DATA_HOME", data.to_str().unwrap()),
        ],
        &["erase", shoot.to_str().unwrap()],
    );

    assert_ok(&out);
    assert!(
        stdout(&out).contains("1 fichier(s) mis à la corbeille, 0 gardé(s)"),
        "stdout: {}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains("mis à la corbeille : "),
        "stdout: {}",
        stdout(&out)
    );
}

// Follow-ups of the #13 review.

#[test]
fn a_trailing_lang_without_a_value_is_refused() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let plan_out = exportpro(&plan_args(&d, &["--shoot", "s", "--lang"]));
    let erase_out = exportpro(&["erase", d.dest.to_str().unwrap(), "--lang"]);

    for out in [plan_out, erase_out] {
        assert_eq!(out.status.code(), Some(1));
        assert!(
            stderr(&out).contains("--lang needs a value"),
            "stderr: {}",
            stderr(&out)
        );
    }
}

#[test]
fn lang_as_the_value_of_another_option_is_that_value() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let out = exportpro(&plan_args(&d, &["--prefix", "--lang", "--shoot", "s"]));

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        ["a.jpg -> 2024/03/05/s/p/o/jpg/--lang_0001.jpg"]
    );
}

#[test]
fn option_errors_are_translated() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let fr = [("LANG", "fr_FR.UTF-8")];
    let src = d.src.to_str().unwrap();

    let no_dest = exportpro_with(&fr, &["plan", "--source", src, "--shoot", "s"]);
    let bad_date = exportpro_with(
        &fr,
        &plan_args(&d, &["--shoot", "s", "--date", "2024-13-01"]),
    );
    let unknown = exportpro_with(&fr, &plan_args(&d, &["--shoot", "s", "--bogus"]));

    assert!(
        stderr(&no_dest).contains("--dest est obligatoire"),
        "{}",
        stderr(&no_dest)
    );
    assert!(
        stderr(&bad_date).contains("--date : 2024-13-01 n'est pas une date (AAAA-MM-JJ)"),
        "{}",
        stderr(&bad_date)
    );
    assert!(
        stderr(&unknown).contains("option inconnue --bogus"),
        "{}",
        stderr(&unknown)
    );
}

#[test]
fn erase_reasons_are_translated() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    assert_ok(&apply(&d, &["--shoot", "s"]));
    let shoot = d.dest.join("2024/03/05/s");
    std::fs::write(shoot.join("p/o/jpg/txo_0001.jpg"), "damaged").unwrap();
    let data = d.home.join(".local/share");

    let out = exportpro_with(
        &[
            ("LANG", "fr_FR.UTF-8"),
            ("HOME", d.home.to_str().unwrap()),
            ("XDG_DATA_HOME", data.to_str().unwrap()),
        ],
        &["erase", shoot.to_str().unwrap()],
    );

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("ne correspond plus au journal"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn the_last_lang_wins() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &[".DS_Store"]);

    let out = exportpro(&plan_args(
        &d,
        &["--shoot", "s", "--lang", "en", "--lang", "fr"],
    ));

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("aucun fichier"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn erase_folders_named_like_plan_options_do_not_hide_lang() {
    let d = Dirs::new();

    let out = exportpro_in(&d.home, &[], &["erase", "--dest", "--lang", "fr"]);

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("--dest/exportpro.log"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        !stderr(&out).contains("fr/exportpro.log"),
        "stderr: {}",
        stderr(&out)
    );
}
