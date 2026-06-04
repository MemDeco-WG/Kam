use crate::cmds::cache::{CacheCommands, TemplateCacheCommands};
use crate::cmds::installed::InstalledCommand;
use crate::cmds::init::KernelSuRepoMode;
use crate::cmds::repo::RepoCommand;
use crate::cmds::secret::SecretCommands;

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

#[test]
fn parses_readable_command_aliases() {
    let singular_tmpl_alias = parse(&["kam", "template", "list"]);
    assert!(matches!(
        singular_tmpl_alias.command,
        Some(Commands::Tmpl(_))
    ));

    let plural_tmpl_alias = parse(&["kam", "templates", "list"]);
    assert!(matches!(plural_tmpl_alias.command, Some(Commands::Tmpl(_))));

    let completion_alias = parse(&["kam", "completion", "bash"]);
    assert!(matches!(
        completion_alias.command,
        Some(Commands::Completions(_))
    ));
}

#[test]
fn parses_kernelsu_reference_init_options() {
    let cli = parse(&[
        "kam",
        "init",
        "org.example.module",
        "--repo-mode",
        "reference",
        "--source-url",
        "https://github.com/example/source",
        "--metamodule",
    ]);
    let Some(Commands::Init(args)) = cli.command else {
        panic!("expected init command");
    };

    assert_eq!(args.repo_mode, KernelSuRepoMode::Reference);
    assert_eq!(
        args.source_url.as_deref(),
        Some("https://github.com/example/source")
    );
    assert!(args.metamodule);
}

#[test]
fn parses_explicit_repo_search_and_download_subcommands() {
    let search = parse(&["kam", "repo", "search", "zygisk", "module"]);
    let Some(Commands::Repo(repo)) = search.command else {
        panic!("expected repo command");
    };
    assert!(matches!(
        repo.command,
        Some(RepoCommand::Search(search_args)) if search_args.query == ["zygisk", "module"]
    ));

    let status = parse(&["kam", "repo", "status", "--quiet"]);
    let Some(Commands::Repo(repo)) = status.command else {
        panic!("expected repo command");
    };
    assert!(matches!(
        repo.command,
        Some(RepoCommand::Status(status_args)) if status_args.quiet
    ));

    let fetch = parse(&["kam", "repo", "fetch", "--yes", "zygisk-next"]);
    let Some(Commands::Repo(repo)) = fetch.command else {
        panic!("expected repo command");
    };
    assert!(matches!(
        repo.command,
        Some(RepoCommand::Fetch(fetch_args))
            if fetch_args.assume_yes && fetch_args.modules == ["zygisk-next"]
    ));

    let download = parse(&["kam", "repo", "download", "--yes", "zygisk-next"]);
    let Some(Commands::Repo(repo)) = download.command else {
        panic!("expected repo command");
    };
    assert!(matches!(
        repo.command,
        Some(RepoCommand::Download(download_args))
            if download_args.assume_yes && download_args.modules == ["zygisk-next"]
    ));

    let info = parse(&["kam", "repo", "info", "MagicNet"]);
    let Some(Commands::Repo(repo)) = info.command else {
        panic!("expected repo command");
    };
    assert!(matches!(
        repo.command,
        Some(RepoCommand::Info(info_args)) if info_args.modules == ["MagicNet"]
    ));

    let list = parse(&["kam", "repo", "list", "magic"]);
    let Some(Commands::Repo(repo)) = list.command else {
        panic!("expected repo command");
    };
    assert!(matches!(
        repo.command,
        Some(RepoCommand::List(list_args)) if list_args.query == ["magic"]
    ));

    let url = parse(&["kam", "repo", "url", "--quiet", "MagicNet"]);
    let Some(Commands::Repo(repo)) = url.command else {
        panic!("expected repo command");
    };
    assert!(matches!(
        repo.command,
        Some(RepoCommand::Url(url_args))
            if url_args.quiet && url_args.modules == ["MagicNet"]
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn parses_explicit_installed_query_subcommands() {
    let list = parse(&["kam", "installed", "list", "--device", "5596d9", "magic"]);
    let Some(Commands::Installed(installed)) = list.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::List(list_args))
            if list_args.device.as_deref() == Some("5596d9") && list_args.query == ["magic"]
    ));

    let search = parse(&["kam", "installed", "search", "zygisk"]);
    let Some(Commands::Installed(installed)) = search.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Search(search_args)) if search_args.query == ["zygisk"]
    ));

    let info = parse(&["kam", "query", "info", "MagicNet"]);
    let Some(Commands::Installed(installed)) = info.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Info(info_args)) if info_args.modules == ["MagicNet"]
    ));

    let upgrades = parse(&["kam", "installed", "upgrades", "--quiet"]);
    let Some(Commands::Installed(installed)) = upgrades.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Upgrades(upgrade_args)) if upgrade_args.quiet
    ));

    let remove = parse(&["kam", "installed", "remove", "--dry-run", "MagicNet"]);
    let Some(Commands::Installed(installed)) = remove.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Remove(remove_args))
            if remove_args.dry_run && remove_args.modules == ["MagicNet"]
    ));

    let foreign = parse(&["kam", "installed", "foreign", "--quiet"]);
    let Some(Commands::Installed(installed)) = foreign.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Foreign(origin_args)) if origin_args.quiet
    ));

    let native = parse(&["kam", "installed", "native"]);
    let Some(Commands::Installed(installed)) = native.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Native(_))
    ));

    let check = parse(&["kam", "installed", "check", "MagicNet"]);
    let Some(Commands::Installed(installed)) = check.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Check(check_args)) if check_args.modules == ["MagicNet"]
    ));

    let owner = parse(&["kam", "installed", "owner", "/data/adb/modules/MagicNet/cli"]);
    let Some(Commands::Installed(installed)) = owner.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Owner(owner_args))
            if owner_args.paths == ["/data/adb/modules/MagicNet/cli"]
    ));

    let files = parse(&["kam", "installed", "files", "MagicNet"]);
    let Some(Commands::Installed(installed)) = files.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::Files(files_args)) if files_args.modules == ["MagicNet"]
    ));

    let package = parse(&["kam", "installed", "package-info", "module.zip"]);
    let Some(Commands::Installed(installed)) = package.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::PackageInfo(package_args))
            if package_args.packages == [std::path::PathBuf::from("module.zip")]
    ));

    let package_files = parse(&["kam", "installed", "package-files", "module.zip"]);
    let Some(Commands::Installed(installed)) = package_files.command else {
        panic!("expected installed command");
    };
    assert!(matches!(
        installed.command,
        Some(InstalledCommand::PackageFiles(package_args))
            if package_args.packages == [std::path::PathBuf::from("module.zip")]
    ));
}

#[test]
fn parses_explicit_template_cache_namespace() {
    let legacy = parse(&["kam", "cache", "list"]);
    let Some(Commands::Cache(cache)) = legacy.command else {
        panic!("expected cache command");
    };
    assert!(matches!(cache.command, CacheCommands::List));

    let namespaced = parse(&["kam", "cache", "templates", "list"]);
    let Some(Commands::Cache(cache)) = namespaced.command else {
        panic!("expected cache command");
    };
    assert!(matches!(
        cache.command,
        CacheCommands::Templates(template_args)
            if matches!(template_args.command, TemplateCacheCommands::List)
    ));
}

#[test]
fn parses_publish_command_options() {
    let cli = parse(&[
        "kam",
        "publish",
        "--repo",
        "KernelSU-Modules-Repo/demo",
        "--tag",
        "v1.0.0",
        "--dist",
        "out",
        "--title",
        "Demo Release",
        "--notes",
        "release notes",
        "--prerelease",
        "--all-assets",
        "--dry-run",
    ]);
    let Some(Commands::Publish(args)) = cli.command else {
        panic!("expected publish command");
    };

    assert_eq!(args.repo.as_deref(), Some("KernelSU-Modules-Repo/demo"));
    assert_eq!(args.tag.as_deref(), Some("v1.0.0"));
    assert_eq!(args.dist, std::path::PathBuf::from("out"));
    assert_eq!(args.title.as_deref(), Some("Demo Release"));
    assert_eq!(args.notes.as_deref(), Some("release notes"));
    assert!(args.prerelease);
    assert!(args.all_assets);
    assert!(args.dry_run);
}
