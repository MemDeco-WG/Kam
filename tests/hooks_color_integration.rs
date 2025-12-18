use serial_test::serial;
use std::process::Command;

/// Integration tests for hook scripts (shell-level).
///
/// These tests spawn a shell, source the `hooks/lib/utils.sh` library, and then
/// use the library-provided variables (like `RED` and `NC`) to produce output
/// that can be asserted against. The goal is to ensure `KAM_COLOR_ERROR` env
/// variable is respected by the hook scripts and results in the expected ANSI
/// color (true-color) sequence being emitted.
#[cfg(unix)]
#[test]
#[serial]
fn hooks_utils_respects_kam_color_error_env() {
    // Choose a test color: #010203 (R=1,G=2,B=3)
    let r = 1u8;
    let g = 2u8;
    let b = 3u8;

    // Expected SGR fragment for truecolor foreground: "38;2;R;G;B"
    let expected_frag = format!("38;2;{};{};{}", r, g, b);

    // Run a shell that
    // 1) has KAM_COLOR_ERROR in its environment,
    // 2) sources the hook library (hooks/lib/utils.sh),
    // 3) prints a formatted error line using the library's RED/NC variables.
    //
    // Using `sh -lc` ensures we run in a POSIX-compatible shell in most CI.
    let output = Command::new("sh")
        .arg("-lc")
        .env("KAM_COLOR_ERROR", "#010203")
        .arg(". hooks/lib/utils.sh && printf '%b' \"${RED}[ERROR]${NC}\\n\"")
        .output()
        .expect("failed to spawn sh");

    assert!(
        output.status.success(),
        "shell command failed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // The output should contain the expected truecolor fragment and the [ERROR] label.
    assert!(
        stdout.contains(&expected_frag),
        "expected ANSI truecolor fragment '{}' in stdout: {}",
        expected_frag,
        stdout
    );
    assert!(
        stdout.contains("[ERROR]"),
        "expected [ERROR] label in stdout: {}",
        stdout
    );
}
