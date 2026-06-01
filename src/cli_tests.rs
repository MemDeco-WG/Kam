use super::{Cli, Commands, inject_double_dash_for_targets};
use crate::cmds::secret::SecretCommands;
use clap::CommandFactory;
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::process::Command;

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from_with_pacman(args).expect("CLI args should parse")
}

fn parse_err(args: &[&str]) -> clap::Error {
    match Cli::try_parse_from_with_pacman(args) {
        Ok(_) => panic!("CLI args should fail to parse"),
        Err(err) => err,
    }
}

#[test]
fn inject_preserves_args_when_double_dash_already_exists() {
    let mut cmd = Cli::command();
    let args = os_args(&["kam", "-S", "--", "target"]);

    assert_eq!(inject_double_dash_for_targets(args.clone(), &mut cmd), args);
}

#[test]
fn injects_after_explicit_sync_before_target() {
    let mut cmd = Cli::command();
    let args = inject_double_dash_for_targets(os_args(&["kam", "-S", "module"]), &mut cmd);

    assert_eq!(args, os_args(&["kam", "-S", "--", "module"]));
}

#[test]
fn injects_after_long_search_before_target() {
    let mut cmd = Cli::command();
    let args = inject_double_dash_for_targets(os_args(&["kam", "--search", "module"]), &mut cmd);

    assert_eq!(args, os_args(&["kam", "--search", "--", "module"]));
}

#[test]
fn injects_after_combined_pacman_flags_before_target() {
    let mut cmd = Cli::command();
    let args = inject_double_dash_for_targets(os_args(&["kam", "-yuS", "module"]), &mut cmd);

    assert_eq!(args, os_args(&["kam", "-yuS", "--", "module"]));
}

#[test]
fn does_not_inject_for_unknown_short_flag() {
    let mut cmd = Cli::command();
    let args = os_args(&["kam", "-Sz", "module"]);

    assert_eq!(inject_double_dash_for_targets(args.clone(), &mut cmd), args);
}

#[test]
fn does_not_inject_before_subcommand_name() {
    let mut cmd = Cli::command();
    let args = os_args(&["kam", "-S", "build"]);

    assert_eq!(inject_double_dash_for_targets(args.clone(), &mut cmd), args);
}

#[test]
fn try_parse_respects_existing_double_dash() {
    let cli = parse(&["kam", "-S", "--", "module"]);

    assert!(cli.sync_flag);
    assert_eq!(cli.targets, vec!["module"]);
}

#[test]
fn try_parse_accepts_combined_sync_update_yes_flags() {
    let cli = parse(&["kam", "-Syu", "module"]);

    assert!(cli.sync_flag);
    assert!(cli.update_index);
    assert!(cli.assume_yes);
    assert_eq!(cli.targets, vec!["module"]);
}

#[test]
fn try_parse_accepts_explicit_search_combo() {
    let cli = parse(&["kam", "-Ss", "term"]);

    assert!(cli.sync_flag);
    assert!(cli.search_flag);
    assert_eq!(cli.targets, vec!["term"]);
}

#[test]
fn parse_from_with_pacman_returns_cli_on_success() {
    let cli = Cli::parse_from_with_pacman(["kam", "--quiet", "build"]);

    assert!(cli.quiet);
    assert!(matches!(cli.command, Some(Commands::Build(_))));
}

#[test]
fn try_parse_reports_invalid_args_with_existing_double_dash() {
    let err = parse_err(&["kam", "--modules-url", "--"]);

    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
}

#[test]
fn try_parse_reports_invalid_args_after_preprocessing() {
    let err = parse_err(&["kam", "--definitely-not-a-real-flag"]);

    assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[test]
fn try_parse_rejects_unknown_combined_short_flag() {
    let err = parse_err(&["kam", "-Sz", "module"]);

    assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[cfg(unix)]
#[test]
fn try_parse_ignores_non_utf8_tokens_during_preprocessing() {
    let args = vec![
        OsString::from("kam"),
        OsString::from_vec(vec![0xff, b'S']),
        OsString::from("build"),
    ];
    let Err(err) = Cli::try_parse_from_with_pacman(args) else {
        panic!("non-UTF8 token should still fail clap parsing");
    };

    assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[cfg(unix)]
#[test]
fn inject_ignores_non_utf8_tokens() {
    let mut cmd = Cli::command();
    let args = vec![
        OsString::from("kam"),
        OsString::from_vec(vec![0xff, b'S']),
        OsString::from("build"),
    ];

    assert_eq!(inject_double_dash_for_targets(args.clone(), &mut cmd), args);
}

#[test]
fn parse_from_with_pacman_exits_on_parse_error() {
    const CHILD_ENV: &str = "KAM_CLI_PARSE_EXIT_CHILD";
    if std::env::var_os(CHILD_ENV).is_some() {
        let _ = Cli::parse_from_with_pacman(["kam", "--definitely-not-a-real-flag"]);
        unreachable!("parse_from_with_pacman should exit on parse errors");
    }

    let current_exe = std::env::current_exe().expect("current test executable");
    let status = Command::new(current_exe)
        .arg("--exact")
        .arg("cli::tests::parse_from_with_pacman_exits_on_parse_error")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .status()
        .expect("spawn parse error child");

    assert_eq!(status.code(), Some(2));
}

#[test]
fn parses_every_top_level_subcommand_variant() {
    type CommandCase<'a> = (&'a [&'a str], fn(&Commands) -> bool);

    let cases: &[CommandCase<'_>] = &[
        (&["kam", "init", "demo"], |cmd| {
            matches!(cmd, Commands::Init(_))
        }),
        (&["kam", "build"], |cmd| matches!(cmd, Commands::Build(_))),
        (&["kam", "version"], |cmd| {
            matches!(cmd, Commands::Version(_))
        }),
        (&["kam", "cache", "list"], |cmd| {
            matches!(cmd, Commands::Cache(_))
        }),
        (&["kam", "tmpl", "list"], |cmd| {
            matches!(cmd, Commands::Tmpl(_))
        }),
        (&["kam", "validate"], |cmd| {
            matches!(cmd, Commands::Validate(_))
        }),
        (&["kam", "completions", "bash"], |cmd| {
            matches!(cmd, Commands::Completions(_))
        }),
        (&["kam", "secret", "list"], |cmd| {
            matches!(cmd, Commands::Secret(_))
        }),
        (&["kam", "sign", "module.zip"], |cmd| {
            matches!(cmd, Commands::Sign(_))
        }),
        (&["kam", "verify", "module.zip"], |cmd| {
            matches!(cmd, Commands::Verify(_))
        }),
        (&["kam", "check"], |cmd| matches!(cmd, Commands::Check(_))),
        (&["kam", "export", "prop"], |cmd| {
            matches!(cmd, Commands::Export(_))
        }),
        (&["kam", "toml", "list"], |cmd| {
            matches!(cmd, Commands::Toml(_))
        }),
        (&["kam", "config", "list"], |cmd| {
            matches!(cmd, Commands::Config(_))
        }),
        (&["kam", "install", "module.zip"], |cmd| {
            matches!(cmd, Commands::Install(_))
        }),
        (&["kam", "repo", "sync"], |cmd| {
            matches!(cmd, Commands::Repo(_))
        }),
        (&["kam", "about"], |cmd| matches!(cmd, Commands::About(_))),
        (&["kam", "env"], |cmd| matches!(cmd, Commands::Env(_))),
        (&["kam", "help"], |cmd| matches!(cmd, Commands::Help(_))),
    ];

    for (args, is_expected_variant) in cases {
        let cli = parse(args);
        let command = cli.command.expect("subcommand should be present");
        assert!(
            is_expected_variant(&command),
            "unexpected command variant for args: {args:?}"
        );
    }
}

#[test]
fn parses_kernel_su_secret_subcommands() {
    let generated = parse(&["kam", "secret", "ksu-generate", "--no-gpg"]);
    let Some(Commands::Secret(secret)) = generated.command else {
        panic!("expected secret command");
    };
    assert!(matches!(
        secret.command,
        Some(SecretCommands::KsuGenerate { no_gpg: true, .. })
    ));

    let submit = parse(&[
        "kam",
        "secret",
        "ksu-submit",
        "--username",
        "octo",
        "--public-key",
        "key.pem",
    ]);
    let Some(Commands::Secret(secret)) = submit.command else {
        panic!("expected secret command");
    };
    assert!(matches!(
        secret.command,
        Some(SecretCommands::KsuSubmit { username, .. }) if username == "octo"
    ));

    let revoke = parse(&[
        "kam",
        "secret",
        "ksu-revoke",
        "--username",
        "octo",
        "--serial-number",
        "01ab",
        "--reason",
        "lost",
    ]);
    let Some(Commands::Secret(secret)) = revoke.command else {
        panic!("expected secret command");
    };
    assert!(matches!(
        secret.command,
        Some(SecretCommands::KsuRevoke {
            serial_number: Some(serial),
            ..
        }) if serial == "01ab"
    ));
}
