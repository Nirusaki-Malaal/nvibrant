// SPDX-License-Identifier: GPL-3.0-only
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_nvibrant")
}
fn run(config: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .env("XDG_CONFIG_HOME", config)
        .env("DISPLAY", "")
        .env("WAYLAND_DISPLAY", "")
        .output()
        .unwrap()
}
#[test]
fn installed_binary_is_self_contained_and_uninstall_preserves_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let prefix = dir.path().join("prefix with spaces");
    let output = run(&config, &["--install", prefix.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = prefix.join("lib/nvibrant");
    assert!(
        fs::read_to_string(payload.join("LICENSE"))
            .unwrap()
            .contains("GNU GENERAL PUBLIC LICENSE")
    );
    assert!(payload.join("licenses/MIT-upstream.txt").is_file());
    assert!(payload.join("NOTICE.md").is_file());
    let installed = prefix.join("bin/nvibrant-gtk");
    let output = Command::new(&installed)
        .arg("--version")
        .current_dir("/")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("2.0.0 (Rust)"));
    let output = Command::new(&installed)
        .arg("--restore")
        .env("XDG_CONFIG_HOME", &config)
        .env("DISPLAY", "")
        .env("WAYLAND_DISPLAY", "")
        .output()
        .unwrap();
    assert!(output.status.success());
    let startup = config.join("autostart/io.github.nvibrant.Aero.Restore.desktop");
    assert!(
        fs::read_to_string(&startup)
            .unwrap()
            .contains(&format!("Exec=\"{}\" --restore", installed.display()))
    );
    let disabled = fs::read_to_string(&startup).unwrap() + "Hidden=true\n";
    fs::write(&startup, &disabled).unwrap();
    assert!(
        run(&config, &["--install", prefix.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(&startup).unwrap(), disabled);
    fs::create_dir_all(config.join("nvibrant")).unwrap();
    let settings = config.join("nvibrant/settings.json");
    fs::write(&settings, "keep").unwrap();
    let unrelated = prefix.join("keep-me");
    fs::write(&unrelated, "keep").unwrap();
    let output = Command::new(&installed)
        .args(["--uninstall", prefix.to_str().unwrap()])
        .env("XDG_CONFIG_HOME", &config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!installed.exists());
    assert!(!startup.exists());
    assert_eq!(fs::read_to_string(settings).unwrap(), "keep");
    assert!(unrelated.exists());
}
#[test]
fn refuses_unrelated_files_and_invalid_cli_before_hardware() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let prefix = dir.path().join("prefix");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::write(prefix.join("bin/nvibrant"), "unrelated").unwrap();
    assert!(
        !run(&config, &["--install", prefix.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(prefix.join("bin/nvibrant")).unwrap(),
        "unrelated"
    );
    for args in [
        vec!["--gpu", "16", "--list-json"],
        vec!["--nonsense"],
        vec!["--apply-json", r#"{"0:0:HDMI":1024}"#],
        vec!["--install", "/"],
    ] {
        let output = run(&config, &args);
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("/dev/nvidia-modeset"));
    }
}
#[test]
fn upgrades_python_payload_without_touching_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let prefix = dir.path().join("prefix");
    let app = prefix.join("lib/nvibrant");
    fs::create_dir_all(app.join("nvibrant")).unwrap();
    fs::write(
        app.join(".nvibrant-install.json"),
        r#"{"application":"nvibrant-glass","python":"/usr/bin/python3"}"#,
    )
    .unwrap();
    fs::write(app.join("nvibrant/gui.py"), "legacy").unwrap();
    assert!(
        run(&config, &["--install", prefix.to_str().unwrap()])
            .status
            .success()
    );
    assert!(!app.join("nvibrant/gui.py").exists());
    assert!(app.join("nvibrant").is_file());
    assert!(
        fs::read_to_string(app.join(".nvibrant-install.json"))
            .unwrap()
            .contains("rust")
    );
}
#[test]
fn unrelated_autostart_is_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(config.join("autostart")).unwrap();
    let path = config.join("autostart/io.github.nvibrant.Aero.Restore.desktop");
    fs::write(&path, "unrelated").unwrap();
    assert!(
        !run(
            &config,
            &["--install", dir.path().join("prefix").to_str().unwrap()]
        )
        .status
        .success()
    );
    assert_eq!(fs::read_to_string(path).unwrap(), "unrelated");
}
