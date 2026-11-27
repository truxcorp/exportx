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
