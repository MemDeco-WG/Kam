use crate::cmds::add::{AddCommands, HookPhase, ScriptPhase, WebuiTemplate};
use crate::cmds::dev::DevCommand;
use crate::cmds::mcp::McpCommand;
use crate::cmds::sync::SyncCommand;

#[test]
fn parses_dev_command_variants() {
    let dev = parse(&[
        "kam",
        "dev",
        "--watch",
        "--hot",
        "--webui",
        "--device",
        "auto",
        "--sync-only",
        "--mcp",
        "--logs",
        "--forward",
        "mcp:webui",
    ]);
    let Some(Commands::Dev(dev)) = dev.command else {
        panic!("expected dev command");
    };
    assert!(dev.watch);
    assert!(dev.hot);
    assert!(dev.webui);
    assert!(dev.sync_only);
    assert!(dev.mcp);
    assert!(dev.logs);
    assert_eq!(dev.device.as_deref(), Some("auto"));
    assert_eq!(dev.forward, vec!["mcp".to_string(), "webui".to_string()]);

    let doctor = parse(&["kam", "dev", "doctor", "--dry-run"]);
    let Some(Commands::Dev(dev)) = doctor.command else {
        panic!("expected dev command");
    };
    assert!(matches!(dev.command, Some(DevCommand::Doctor)));
    assert!(dev.dry_run);

    let doctor_with_device = parse(&["kam", "dev", "--device", "5596d9", "doctor"]);
    let Some(Commands::Dev(dev)) = doctor_with_device.command else {
        panic!("expected dev command");
    };
    assert!(matches!(dev.command, Some(DevCommand::Doctor)));
    assert_eq!(dev.device.as_deref(), Some("5596d9"));
}

#[test]
fn parses_mcp_command_variants() {
    let status = parse(&["kam", "mcp", "--device", "abc", "status", "--json"]);
    let Some(Commands::Mcp(mcp)) = status.command else {
        panic!("expected mcp command");
    };
    assert_eq!(mcp.device.as_deref(), Some("abc"));
    assert!(matches!(mcp.command, McpCommand::Status { json: true }));

    let forward = parse(&["kam", "mcp", "--local-port", "9876", "forward"]);
    let Some(Commands::Mcp(mcp)) = forward.command else {
        panic!("expected mcp command");
    };
    assert_eq!(mcp.local_port, Some(9876));
    assert!(matches!(mcp.command, McpCommand::Forward));
}

#[test]
fn parses_add_command_variants() {
    let script = parse(&["kam", "add", "script", "service", "--dry-run"]);
    let Some(Commands::Add(add)) = script.command else {
        panic!("expected add command");
    };
    assert!(matches!(
        add.command,
        AddCommands::Script {
            phase: ScriptPhase::Service,
            dry_run: true,
            ..
        }
    ));

    let hook = parse(&[
        "kam",
        "add",
        "hook",
        "pre-build",
        "sync-version",
        "--order",
        "20",
    ]);
    let Some(Commands::Add(add)) = hook.command else {
        panic!("expected add command");
    };
    assert!(matches!(
        add.command,
        AddCommands::Hook {
            phase: HookPhase::PreBuild,
            order: 20,
            ..
        }
    ));

    let kamfw = parse(&["kam", "add", "kamfw", "watchdog", "--phase", "service"]);
    let Some(Commands::Add(add)) = kamfw.command else {
        panic!("expected add command");
    };
    assert!(matches!(
        add.command,
        AddCommands::Kamfw {
            ref module,
            phase: ScriptPhase::Service,
            ..
        } if module == "watchdog"
    ));

    let webui = parse(&["kam", "add", "webui", "--template", "static"]);
    let Some(Commands::Add(add)) = webui.command else {
        panic!("expected add command");
    };
    assert!(matches!(
        add.command,
        AddCommands::Webui {
            template: WebuiTemplate::Static,
            ..
        }
    ));
}

#[test]
fn parses_shell_build_optimization_flags() {
    let build = parse(&[
        "kam",
        "build",
        "--trim-shell-functions",
        "--obfuscate-shell",
        "--quiet",
    ]);
    let Some(Commands::Build(build)) = build.command else {
        panic!("expected build command");
    };
    assert!(build.trim_shell_functions);
    assert!(!build.trim_shell);
    assert!(build.obfuscate_shell);
}

#[test]
fn parses_sync_command_variants() {
    let default_sync = parse(&["kam", "sync", "--dry-run"]);
    let Some(Commands::Sync(sync)) = default_sync.command else {
        panic!("expected sync command");
    };
    assert!(sync.dry_run);
    assert!(sync.command.is_none());

    let workflow = parse(&[
        "kam",
        "sync",
        "workflow",
        "--source-repo",
        "owner/repo",
        "--check",
    ]);
    let Some(Commands::Sync(sync)) = workflow.command else {
        panic!("expected sync command");
    };
    assert!(sync.check);
    assert!(matches!(
        sync.command,
        Some(SyncCommand::Workflow {
            source_repo: Some(ref source_repo)
        }) if source_repo == "owner/repo"
    ));

    let all = parse(&["kam", "sync", "--remote", "all"]);
    let Some(Commands::Sync(sync)) = all.command else {
        panic!("expected sync command");
    };
    assert!(sync.remote);
    assert!(matches!(sync.command, Some(SyncCommand::All { .. })));
}
