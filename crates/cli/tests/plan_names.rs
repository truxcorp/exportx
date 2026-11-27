//! Behaviour 7: names invalid on any OS are refused (ported and extended from
//! tests/test_exportpro.sh, case 6).
mod common;
use common::*;

const BAD: &[&str] = &[
    "",
    "a/b",
    "a\\b",
    "a:b",
    "a*b",
    "a?b",
    "a\"b",
    "a<b",
    "a>b",
    "a|b",
    "a\tb",
    "a\u{7}b",
    "CON",
    "con",
    "Nul.txt",
    "COM1",
    "lpt9",
    "trailing.",
    "trailing ",
    ".hidden",
];

fn refused(flag: &str, name: &str) -> Result<(), String> {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let mut args = vec!["--shoot", "s", "--prefix", "p"];
    let i = args.iter().position(|a| *a == flag).unwrap();
    args[i + 1] = name;
    let out = plan(&d, &args);
    let err = stderr(&out);
    if out.status.code() == Some(1) && moves(&out).is_empty() && err.contains("name") {
        Ok(())
    } else {
        Err(format!(
            "{flag} {name:?}: exit {:?}, stderr {err:?}",
            out.status.code()
        ))
    }
}

#[test]
fn invalid_shoot_names_are_refused() {
    let failures: Vec<String> = BAD
        .iter()
        .filter_map(|n| refused("--shoot", n).err())
        .collect();
    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn invalid_prefixes_are_refused() {
    let failures: Vec<String> = BAD
        .iter()
        .filter_map(|n| refused("--prefix", n).err())
        .collect();
    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn names_valid_everywhere_are_accepted() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let out = plan(
        &d,
        &["--shoot", "Mariage Dupont – été 2024", "--prefix", "md-24"],
    );

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        ["a.jpg -> 2024/03/05/Mariage Dupont – été 2024/p/o/jpg/md-24_0001.jpg"]
    );
}

#[test]
fn destinations_differing_only_by_case_are_refused() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.JPG", "a.jpg"]);

    let out = plan(&d, &["--shoot", "s"]);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert_eq!(moves(&out), Vec::<String>::new());
    assert!(
        stderr(&out).contains("a.JPG") && stderr(&out).contains("a.jpg"),
        "stderr: {}",
        stderr(&out)
    );
}
