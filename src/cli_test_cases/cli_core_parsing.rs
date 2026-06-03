use super::{Cli, Commands, inject_double_dash_for_targets};
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
fn injects_after_explicit_query_before_target() {
    let mut cmd = Cli::command();
    let args = inject_double_dash_for_targets(os_args(&["kam", "-Q", "module"]), &mut cmd);

    assert_eq!(args, os_args(&["kam", "-Q", "--", "module"]));
}

#[test]
fn injects_after_explicit_local_install_before_target() {
    let mut cmd = Cli::command();
    let args = inject_double_dash_for_targets(os_args(&["kam", "-U", "module.zip"]), &mut cmd);

    assert_eq!(args, os_args(&["kam", "-U", "--", "module.zip"]));
}

#[test]
fn injects_after_explicit_remove_before_target() {
    let mut cmd = Cli::command();
    let args = inject_double_dash_for_targets(os_args(&["kam", "-R", "module"]), &mut cmd);

    assert_eq!(args, os_args(&["kam", "-R", "--", "module"]));
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
fn try_parse_accepts_sync_clean_combo() {
    let cli = parse(&["kam", "-Sc"]);

    assert!(cli.sync_flag);
    assert!(cli.targets.is_empty());
}

#[test]
fn try_parse_accepts_explicit_search_combo() {
    let cli = parse(&["kam", "-Ss", "term"]);

    assert!(cli.sync_flag);
    assert!(cli.search_flag);
    assert_eq!(cli.targets, vec!["term"]);
}

#[test]
fn try_parse_accepts_explicit_installed_query_search_combo() {
    let cli = parse(&["kam", "-Qs", "term"]);

    assert!(cli.query_flag);
    assert!(cli.search_flag);
    assert_eq!(cli.targets, vec!["term"]);
}

#[test]
fn try_parse_accepts_installed_query_info_combo() {
    let cli = parse(&["kam", "-Qi", "MagicNet"]);

    assert!(cli.query_flag);
    assert!(cli.info_flag);
    assert_eq!(cli.targets, vec!["MagicNet"]);
}

#[test]
fn try_parse_accepts_installed_query_upgrade_combo() {
    let cli = parse(&["kam", "-Qu"]);

    assert!(cli.query_flag);
    assert!(cli.update_index);
    assert!(cli.targets.is_empty());
}

#[test]
fn try_parse_accepts_installed_query_foreign_combo() {
    let cli = parse(&["kam", "-Qm"]);

    assert!(cli.query_flag);
    assert!(cli.foreign_flag);
}

#[test]
fn try_parse_accepts_installed_query_native_combo() {
    let cli = parse(&["kam", "-Qn"]);

    assert!(cli.query_flag);
    assert!(cli.native_flag);
}

#[test]
fn try_parse_accepts_installed_query_check_combo() {
    let cli = parse(&["kam", "-Qk", "MagicNet"]);

    assert!(cli.query_flag);
    assert!(cli.check_flag);
    assert_eq!(cli.targets, ["MagicNet"]);
}

#[test]
fn try_parse_accepts_installed_query_owner_combo() {
    let cli = parse(&["kam", "-Qo", "/data/adb/modules/MagicNet/cli"]);

    assert!(cli.query_flag);
    assert!(cli.owner_flag);
    assert_eq!(cli.targets, ["/data/adb/modules/MagicNet/cli"]);
}

#[test]
fn try_parse_accepts_installed_query_files_combo() {
    let cli = parse(&["kam", "-Ql", "MagicNet"]);

    assert!(cli.query_flag);
    assert!(cli.list_flag);
    assert_eq!(cli.targets, ["MagicNet"]);
}

#[test]
fn try_parse_accepts_installed_package_query_combo() {
    let cli = parse(&["kam", "-Qp", "module.zip"]);

    assert!(cli.query_flag);
    assert!(cli.package_flag);
    assert_eq!(cli.targets, ["module.zip"]);
}

#[test]
fn try_parse_accepts_installed_package_file_list_combo() {
    let cli = parse(&["kam", "-Qpl", "module.zip"]);

    assert!(cli.query_flag);
    assert!(cli.package_flag);
    assert!(cli.list_flag);
    assert_eq!(cli.targets, ["module.zip"]);
}

#[test]
fn try_parse_accepts_local_install_flag() {
    let cli = parse(&["kam", "-U", "module.zip"]);

    assert!(cli.local_install_flag);
    assert_eq!(cli.targets, vec!["module.zip"]);
}

#[test]
fn try_parse_accepts_local_install_with_adb_options() {
    let cli = parse(&[
        "kam",
        "-U",
        "module.zip",
        "--adb",
        "--manager",
        "KernelSU",
        "--dry-run",
    ]);

    assert!(cli.local_install_flag);
    assert_eq!(
        cli.targets,
        vec!["module.zip", "--adb", "--manager", "KernelSU", "--dry-run"]
    );
}

#[test]
fn try_parse_accepts_remove_flag() {
    let cli = parse(&["kam", "-R", "MagicNet"]);

    assert!(cli.remove_flag);
    assert_eq!(cli.targets, vec!["MagicNet"]);
}

#[test]
fn try_parse_accepts_remove_with_dry_run_device() {
    let cli = parse(&[
        "kam",
        "-R",
        "MagicNet",
        "--dry-run",
        "--device",
        "5596d9",
    ]);

    assert!(cli.remove_flag);
    assert_eq!(cli.targets, vec!["MagicNet", "--dry-run", "--device", "5596d9"]);
}

#[test]
fn try_parse_accepts_global_device_for_query() {
    let cli = parse(&["kam", "--device", "5596d9", "-Q"]);

    assert!(cli.query_flag);
    assert_eq!(cli.device.as_deref(), Some("5596d9"));
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
        (&["kam", "add", "script", "service"], |cmd| {
            matches!(cmd, Commands::Add(_))
        }),
        (&["kam", "dev", "--sync-only"], |cmd| {
            matches!(cmd, Commands::Dev(_))
        }),
        (&["kam", "diff", "--device", "abc"], |cmd| {
            matches!(cmd, Commands::Diff(_))
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
        (&["kam", "sync"], |cmd| matches!(cmd, Commands::Sync(_))),
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
        (&["kam", "installed", "list"], |cmd| {
            matches!(cmd, Commands::Installed(_))
        }),
        (&["kam", "mcp", "status"], |cmd| {
            matches!(cmd, Commands::Mcp(_))
        }),
        (&["kam", "publish", "--dry-run"], |cmd| {
            matches!(cmd, Commands::Publish(_))
        }),
        (&["kam", "workflow", "install", "owner/repo"], |cmd| {
            matches!(cmd, Commands::Workflow(_))
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
fn parses_pacman_targets_with_trailing_global_flags() {
    let cli = parse(&["kam", "-Syu", "MagicNet", "--yes", "-q"]);

    assert!(cli.sync_flag);
    assert_eq!(cli.targets, vec!["MagicNet", "--yes", "-q"]);
}

#[test]
fn parses_pacman_info_and_list_flags() {
    let info = parse(&["kam", "-Si", "MagicNet"]);
    assert!(info.sync_flag);
    assert_eq!(info.targets, vec!["MagicNet"]);

    let list = parse(&["kam", "-Sl", "magic"]);
    assert!(list.sync_flag);
    assert_eq!(list.targets, vec!["magic"]);
}
