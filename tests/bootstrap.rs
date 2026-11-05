// SPDX-License-Identifier: GPL-3.0-only
//! Exercise the curl entry point without downloading code or installing packages.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let curl = bin.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
while [ "$#" -gt 0 ]; do
    case $1 in
        -o) destination=$2; shift 2 ;;
        https://*) printf '%s\n' "$1" > "$NVIBRANT_TEST_URL"; shift ;;
        *) shift ;;
    esac
done
[ "${NVIBRANT_TEST_FAIL:-0}" = 0 ] || exit 22
cp "$NVIBRANT_TEST_FIXTURE" "$destination"
"#,
    )
    .unwrap();
    fs::set_permissions(curl, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        dir.path().join("build-fixture.sh"),
        "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\n",
    )
    .unwrap();
    (dir, bin)
}
fn run(root: &Path, bin: &Path, args: &[&str], fail: bool) -> Output {
    Command::new("/bin/sh")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh"))
        .args(args)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("NVIBRANT_TEST_URL", root.join("url"))
        .env("NVIBRANT_TEST_FIXTURE", root.join("build-fixture.sh"))
        .env("NVIBRANT_TEST_FAIL", if fail { "1" } else { "0" })
        .env_remove("NVIBRANT_REF")
        .output()
        .unwrap()
}
#[test]
fn default_curl_install_prepares_dependencies_and_preserves_arguments() {
    let (dir, bin) = fixture();
    let output = run(
        dir.path(),
        &bin,
        &["--prefix", "/tmp/prefix with spaces"],
        false,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "<install>\n<--install-deps>\n<--prefix>\n</tmp/prefix with spaces>\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("url")).unwrap(),
        "https://raw.githubusercontent.com/Nirusaki-Malaal/nvibrant/main/scripts/build.sh\n"
    );
}
#[test]
fn no_deps_and_source_ref_are_forwarded_correctly() {
    let (dir, bin) = fixture();
    let output = run(dir.path(), &bin, &["--no-deps", "--ref", "v2.0.0"], false);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "<install>\n<--ref>\n<v2.0.0>\n"
    );
    assert!(
        fs::read_to_string(dir.path().join("url"))
            .unwrap()
            .contains("/v2.0.0/scripts/build.sh")
    );
}
#[test]
fn help_and_invalid_refs_do_not_download_or_run_a_build() {
    let (dir, bin) = fixture();
    assert!(run(dir.path(), &bin, &["--help"], false).status.success());
    assert!(!dir.path().join("url").exists());
    assert!(
        !run(dir.path(), &bin, &["--ref", "bad ref"], false)
            .status
            .success()
    );
    assert!(!run(dir.path(), &bin, &["--ref"], false).status.success());
    assert!(!dir.path().join("url").exists());
}
#[test]
fn failed_download_never_runs_the_build() {
    let (dir, bin) = fixture();
    let output = run(dir.path(), &bin, &[], true);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn outdated_rust_gets_a_private_toolchain_without_replacing_the_existing_one() {
    let (dir, bin) = fixture();
    let executable = |name: &str, text: &str| {
        let path = bin.join(name);
        fs::write(&path, text).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    };
    executable("id", "#!/bin/sh\nprintf '0\\n'\n");
    executable("apt-get", "#!/bin/sh\nexit 0\n");
    executable("pkg-config", "#!/bin/sh\nexit 0\n");
    executable("clang", "#!/bin/sh\nexit 0\n");
    let old = "#!/bin/sh\nprintf 'rustc 1.70.0 (fixture)\\n'\n";
    executable("rustc", old);
    executable("cargo", "#!/bin/sh\nexit 0\n");
    fs::write(
        dir.path().join("build-fixture.sh"),
        r#"#!/bin/sh
printf '%s\n' "$@" > "$NVIBRANT_TEST_RUSTUP_ARGS"
mkdir -p "$CARGO_HOME/bin"
printf '#!/bin/sh\nprintf "rustc 1.85.0 (fixture)\\n"\n' > "$CARGO_HOME/bin/rustc"
printf '#!/bin/sh\nexit 0\n' > "$CARGO_HOME/bin/cargo"
chmod +x "$CARGO_HOME/bin/rustc" "$CARGO_HOME/bin/cargo"
"#,
    )
    .unwrap();
    let output = Command::new("/bin/bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/build.sh"))
        .args(["check", "--install-deps"])
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("XDG_CACHE_HOME", dir.path().join("cache"))
        .env("NVIBRANT_TEST_URL", dir.path().join("url"))
        .env("NVIBRANT_TEST_FIXTURE", dir.path().join("build-fixture.sh"))
        .env("NVIBRANT_TEST_RUSTUP_ARGS", dir.path().join("rustup-args"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(bin.join("rustc")).unwrap(), old);
    assert!(
        dir.path()
            .join("cache/nvibrant/toolchain/cargo/bin/rustc")
            .exists()
    );
    assert!(
        fs::read_to_string(dir.path().join("rustup-args"))
            .unwrap()
            .contains("--no-modify-path")
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("url")).unwrap(),
        "https://sh.rustup.rs\n"
    );
}
