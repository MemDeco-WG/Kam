// Integration tests for CLI parsing of combined short flags (e.g. `-Syu`).
// These ensure that multiple short options concatenated into a single token
// are parsed as separate flags (sync, yes, update).
use clap::Parser;
use kam::cli::Cli;

#[test]
fn parsing_combined_short_flags_sets_flags() {
    let cli = Cli::try_parse_from_with_pacman(["kam", "-Syu"]).unwrap();

    assert!(cli.sync, "expected -S (sync) to be true");
    assert!(cli.assume_yes, "expected -y (assume yes) to be true");
    assert!(cli.update_index, "expected -u (update index) to be true");
    assert!(cli.targets.is_empty(), "expected no targets");
    assert!(cli.command.is_none(), "expected no subcommand");
}

#[test]
fn parsing_combined_short_flags_with_targets() {
    let cli =
        Cli::try_parse_from_with_pacman(["kam", "-Syu", "mod.example", "another_mod"]).unwrap();

    assert!(cli.sync);
    assert!(cli.assume_yes);
    assert!(cli.update_index);
    assert_eq!(
        cli.targets,
        vec!["mod.example".to_string(), "another_mod".to_string()]
    );
    assert!(cli.command.is_none());
}

#[test]
fn parsing_combined_short_flags_search_modifier() {
    // -Ss should be interpreted as -S and -s together (search mode)
    let cli = Cli::try_parse_from_with_pacman(["kam", "-Ss", "search_term"]).unwrap();

    assert!(cli.sync, "expected -S (sync) to be true");
    assert!(cli.search, "expected -s (search) to be true");
    assert_eq!(cli.targets, vec!["search_term".to_string()]);
}

#[test]
fn parsing_combined_short_flags_different_order() {
    // Order of concatenated short flags should not matter
    let cli = Cli::try_parse_from_with_pacman(["kam", "-yuS", "modid"]).unwrap();

    assert!(cli.sync);
    assert!(cli.assume_yes);
    assert!(cli.update_index);
    assert_eq!(cli.targets, vec!["modid".to_string()]);
}
