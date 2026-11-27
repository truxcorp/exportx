//! Behaviour 15: the last destination, prefix and layout preset are remembered.
mod common;
use common::*;

use std::process::Output;

fn run(d: &Dirs, args: &[&str]) -> Output {
    let config = d.home.join(".config");
    exportpro_with(&[("XDG_CONFIG_HOME", config.to_str().unwrap())], args)
}

fn without_dest<'a>(cmd: &'a str, d: &'a Dirs, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![cmd, "--source", d.src.to_str().unwrap()];
    args.extend_from_slice(extra);
    args
}

#[test]
fn a_copy_remembers_destination_prefix_and_preset() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let first = run(
        &d,
        &command_args(
            "apply",
            &d,
            &["--shoot", "s", "--prefix", "md", "--preset", "lisible"],
        ),
    );
    assert_ok(&first);
    mk(&d.src, "2024-03-05 10:00", &["b.jpg"]);

    let out = run(&d, &without_dest("plan", &d, &["--shoot", "t"]));

    assert_ok(&out);
    let dest = d.dest.to_str().unwrap();
    assert!(
        stdout(&out).contains("b.jpg -> 2024/03/05/t/photo/originaux/jpg/md_0002.jpg"),
        "stdout: {}",
        stdout(&out)
    );
    assert!(!stdout(&out).contains(dest), "plan lines are relative");
}

#[test]
fn options_given_win_over_remembered_ones() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    assert_ok(&run(
        &d,
        &command_args(
            "apply",
            &d,
            &["--shoot", "s", "--prefix", "md", "--preset", "lisible"],
        ),
    ));

    let out = run(
        &d,
        &command_args(
            "plan",
            &d,
            &["--shoot", "t", "--prefix", "zz", "--preset", "classique"],
        ),
    );

    assert_ok(&out);
    assert_eq!(moves(&out), ["a.jpg -> 2024/03/05/t/p/o/jpg/zz_0001.jpg"]);
}

#[test]
fn a_plan_alone_remembers_nothing() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    assert_ok(&run(
        &d,
        &command_args("plan", &d, &["--shoot", "s", "--prefix", "md"]),
    ));

    let out = run(&d, &without_dest("plan", &d, &["--shoot", "s"]));

    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("--dest"), "stderr: {}", stderr(&out));
}

// Review of PR #13: reproductions.

#[test]
fn a_relative_destination_is_remembered_absolute() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let config = d.home.join(".config");
    let env = [("XDG_CONFIG_HOME", config.to_str().unwrap())];
    let parent = d.dest.parent().unwrap();
    let src = d.src.to_str().unwrap();
    assert_ok(&exportpro_in(
        parent,
        &env,
        &["apply", "--source", src, "--dest", "dest", "--shoot", "s"],
    ));
    mk(&d.src, "2024-03-05 10:00", &["b.jpg"]);

    let out = exportpro_in(&d.home, &env, &["apply", "--source", src, "--shoot", "s"]);

    assert_ok(&out);
    assert!(
        d.dest.join("2024/03/05/s/p/o/jpg/txo_0002.jpg").is_file(),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn custom_templates_without_a_preset_start_from_classique() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    assert_ok(&run(
        &d,
        &command_args("apply", &d, &["--shoot", "s", "--preset", "lisible"]),
    ));

    let out = run(
        &d,
        &command_args(
            "plan",
            &d,
            &["--shoot", "t", "--shoot-path", "{yyyy}/{shoot}"],
        ),
    );

    assert_ok(&out);
    assert_eq!(moves(&out), ["a.jpg -> 2024/t/p/o/jpg/txo_0001.jpg"]);
}

#[test]
fn settings_live_in_the_exportx_folder_and_exportpro_ones_are_picked_up() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let config = d.home.join(".config");
    // As ExportPro left them.
    std::fs::create_dir_all(config.join("exportpro")).unwrap();
    let old = format!("dest = {:?}\nprefix = \"md\"\n", d.dest.to_str().unwrap());
    std::fs::write(config.join("exportpro/settings.toml"), old).unwrap();

    let out = run(&d, &without_dest("apply", &d, &["--shoot", "s"]));

    assert_ok(&out);
    assert!(stdout(&out).contains("md_0001.jpg"), "{}", stdout(&out));
    let saved = std::fs::read_to_string(config.join("exportx/settings.toml")).unwrap();
    assert!(saved.contains("prefix = \"md\""), "{saved}");
}
