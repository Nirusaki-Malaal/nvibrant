use crate::settings;
use anyhow::{Context, Result, ensure};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::Command,
};
const ID: &str = "io.github.nvibrant.Aero";
const MARKER: &str = ".nvibrant-install.json";
fn exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}
fn autostart() -> PathBuf {
    settings::config_root().join(format!("autostart/{ID}.Restore.desktop"))
}
fn quoted(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%")
    )
}
fn owned(prefix: &Path) -> Result<()> {
    ensure!(
        prefix.is_absolute()
            && prefix != Path::new("/")
            && !prefix.to_string_lossy().contains(['\n', '\r']),
        "Prefix must be an absolute directory other than /"
    );
    let app = prefix.join("lib/nvibrant");
    if exists(&app) {
        ensure!(
            !app.symlink_metadata()?.file_type().is_symlink(),
            "Refusing a symlink payload directory"
        );
        let marker: serde_json::Value = serde_json::from_slice(
            &fs::read(app.join(MARKER)).context("Unrecognized installation directory")?,
        )?;
        ensure!(
            marker["application"] == "nvibrant-glass",
            "Unrecognized installation directory"
        );
    } else {
        for path in [
            prefix.join("bin/nvibrant"),
            prefix.join("bin/nvibrant-gtk"),
            prefix.join(format!("share/applications/{ID}.desktop")),
            prefix.join(format!("share/icons/hicolor/scalable/apps/{ID}.svg")),
        ] {
            ensure!(
                !exists(&path),
                "Refusing to overwrite unrelated file {}",
                path.display()
            );
        }
    }
    Ok(())
}
fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().context("Missing parent directory")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(contents)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn refresh(prefix: &Path) {
    for (program, args) in [
        (
            "update-desktop-database",
            vec![prefix.join("share/applications").into_os_string()],
        ),
        (
            "gtk-update-icon-cache",
            vec![
                "--force".into(),
                "--ignore-theme-index".into(),
                prefix.join("share/icons/hicolor").into_os_string(),
            ],
        ),
    ] {
        let _ = Command::new(program)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}
enum Snapshot {
    Missing,
    Link(PathBuf),
    File(Vec<u8>, fs::Permissions),
}
impl Snapshot {
    fn capture(path: &Path) -> Result<Self> {
        match path.symlink_metadata() {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::Missing),
            Err(error) => Err(error.into()),
            Ok(meta) if meta.file_type().is_symlink() => Ok(Self::Link(fs::read_link(path)?)),
            Ok(meta) => {
                ensure!(
                    meta.is_file(),
                    "Refusing to replace non-file {}",
                    path.display()
                );
                Ok(Self::File(fs::read(path)?, meta.permissions()))
            }
        }
    }
    fn restore(&self, path: &Path) -> Result<()> {
        if exists(path) {
            fs::remove_file(path)?;
        }
        match self {
            Self::Missing => {}
            Self::Link(target) => symlink(target, path)?,
            Self::File(data, permissions) => {
                write_atomic(path, data)?;
                fs::set_permissions(path, permissions.clone())?;
            }
        }
        Ok(())
    }
}
pub fn install(prefix: &Path) -> Result<()> {
    owned(prefix)?;
    let app = prefix.join("lib/nvibrant");
    let gui = prefix.join("bin/nvibrant-gtk");
    let startup = autostart();
    let owner = format!("X-NVibrant-Launcher={}\n", gui.display());
    if exists(&startup) {
        ensure!(
            fs::read_to_string(&startup)?.contains(&owner),
            "Refusing to replace unrelated autostart entry {}",
            startup.display()
        );
    }
    fs::create_dir_all(prefix.join("lib"))?;
    fs::create_dir_all(prefix.join("bin"))?;
    let backup = prefix.join("lib/.nvibrant-previous");
    ensure!(
        !exists(&backup),
        "An interrupted upgrade exists at {}; restore or move it first",
        backup.display()
    );
    let staging = tempfile::Builder::new()
        .prefix(".nvibrant-install-")
        .tempdir_in(prefix.join("lib"))?;
    fs::copy(std::env::current_exe()?, staging.path().join("nvibrant"))?;
    fs::set_permissions(
        staging.path().join("nvibrant"),
        fs::Permissions::from_mode(0o755),
    )?;
    fs::write(
        staging.path().join(MARKER),
        serde_json::to_vec_pretty(
            &json!({"application":"nvibrant-glass","language":"rust","version":env!("CARGO_PKG_VERSION")}),
        )?,
    )?;
    fs::write(
        staging.path().join("license.txt"),
        include_str!("../license.txt"),
    )?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=NVibrant\nComment=Tool for Wayland to change vibrancy on NVIDIA GPUs.\nExec={}\nIcon={ID}\nTerminal=false\nCategories=Settings;HardwareSettings;\nKeywords=NVIDIA;Vibrance;Display;Color;\n",
        quoted(&gui)
    );
    let restore = format!(
        "[Desktop Entry]\nType=Application\nName=NVibrant saved colours\nExec={} --restore\nTerminal=false\n{owner}",
        quoted(&gui)
    );
    let paths = [
        prefix.join("bin/nvibrant"),
        gui.clone(),
        prefix.join(format!("share/applications/{ID}.desktop")),
        prefix.join(format!("share/icons/hicolor/scalable/apps/{ID}.svg")),
        startup.clone(),
    ];
    let snapshots = paths
        .iter()
        .map(|path| Snapshot::capture(path))
        .collect::<Result<Vec<_>>>()?;
    let upgraded = app.exists();
    if upgraded {
        fs::rename(&app, &backup)?;
    }
    let result = (|| -> Result<()> {
        fs::rename(staging.path(), &app)?;
        for name in ["nvibrant", "nvibrant-gtk"] {
            let path = prefix.join("bin").join(name);
            if exists(&path) {
                fs::remove_file(&path)?;
            }
            symlink("../lib/nvibrant/nvibrant", &path)?;
        }
        write_atomic(
            &prefix.join(format!("share/applications/{ID}.desktop")),
            entry.as_bytes(),
        )?;
        write_atomic(
            &prefix.join(format!("share/icons/hicolor/scalable/apps/{ID}.svg")),
            include_bytes!("../assets/io.github.nvibrant.Aero.svg"),
        )?;
        if !exists(&startup) {
            write_atomic(&startup, restore.as_bytes())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        // Restore both the previous payload and every managed file, including
        // executable shell launchers when upgrading from the Python version.
        if app.exists() {
            fs::remove_dir_all(&app)?;
        }
        if upgraded {
            fs::rename(&backup, &app)?;
        }
        for (path, snapshot) in paths.iter().zip(&snapshots) {
            snapshot.restore(path)?;
        }
        return Err(error);
    }
    if upgraded {
        fs::remove_dir_all(backup)?;
    }
    refresh(prefix);
    println!(
        "Installed NVibrant {} (Rust) into {}\nLaunch: {}\nSaved colours restore at login: {}",
        env!("CARGO_PKG_VERSION"),
        prefix.display(),
        gui.display(),
        startup.display()
    );
    Ok(())
}
pub fn uninstall(prefix: &Path) -> Result<()> {
    owned(prefix)?;
    let app = prefix.join("lib/nvibrant");
    if !app.exists() {
        println!("No NVibrant installation at {}", prefix.display());
        return Ok(());
    }
    let startup = autostart();
    if exists(&startup)
        && fs::read_to_string(&startup)?.contains(&format!(
            "X-NVibrant-Launcher={}\n",
            prefix.join("bin/nvibrant-gtk").display()
        ))
    {
        fs::remove_file(startup)?;
    }
    for path in [
        prefix.join("bin/nvibrant"),
        prefix.join("bin/nvibrant-gtk"),
        prefix.join(format!("share/applications/{ID}.desktop")),
        prefix.join(format!("share/icons/hicolor/scalable/apps/{ID}.svg")),
    ] {
        if exists(&path) {
            fs::remove_file(path)?;
        }
    }
    fs::remove_dir_all(app)?;
    refresh(prefix);
    println!("Uninstalled NVibrant; saved preferences preserved");
    Ok(())
}
