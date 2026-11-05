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
        "https://raw.githubusercontent.com/Nirusaki-Malaal/nvibrant/main/build.sh\n"
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
            .contains("/v2.0.0/build.sh")
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
