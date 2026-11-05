// SPDX-License-Identifier: GPL-3.0-only
use crate::backend::Port;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub fn config_root() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
}
pub fn path() -> PathBuf {
    config_root().join("nvibrant/settings.json")
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub version: u8,
    pub last_gpu: u8,
    pub gpus: BTreeMap<String, BTreeMap<String, i32>>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            last_gpu: 0,
            gpus: BTreeMap::new(),
        }
    }
}
pub fn valid_key(key: &str) -> bool {
    let parts: Vec<_> = key.split(':').collect();
    parts.len() == 3
        && parts[..2]
            .iter()
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        && !parts[2].is_empty()
        && parts[2]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
pub fn validate_values(values: &BTreeMap<String, i32>) -> Result<()> {
    for (key, value) in values {
        ensure!(
            valid_key(key) && (-1024..=1023).contains(value),
            "Invalid display setting: {key}"
        );
    }
    Ok(())
}
impl Settings {
    pub fn load(path: &Path) -> Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).with_context(|| format!("Cannot read {}", path.display())),
        };
        let data: Self = serde_json::from_str(&text)
            .with_context(|| format!("Invalid settings at {}", path.display()))?;
        ensure!(
            data.version == 1 && data.last_gpu <= 15,
            "Unsupported settings format"
        );
        for (gpu, values) in &data.gpus {
            let index: u8 = gpu.parse().context("Invalid GPU index")?;
            ensure!(
                index <= 15 && index.to_string() == *gpu,
                "Invalid GPU index"
            );
            validate_values(values)?;
        }
        Ok(data)
    }
    pub fn value(&self, gpu: u8, port: &Port) -> Option<i32> {
        self.gpus.get(&gpu.to_string())?.get(&port.key()).copied()
    }
    pub fn save(path: &Path, gpu: u8, values: &BTreeMap<String, i32>) -> Result<()> {
        ensure!(gpu <= 15, "Invalid GPU index");
        validate_values(values)?;
        let parent = path.parent().context("Settings need a parent directory")?;
        fs::create_dir_all(parent)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(parent.join(".settings.lock"))?;
        lock.lock_exclusive()?;
        let mut data = Self::load(path)?;
        data.gpus
            .entry(gpu.to_string())
            .or_default()
            .extend(values.clone());
        data.last_gpu = gpu;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, &data)?;
        writeln!(temporary)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|e| e.error)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_python_preferences_and_preserves_other_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"version":1,"last_gpu":0,"gpus":{"0":{"0:5:DisplayPort":1023}}}"#,
        )
        .unwrap();
        Settings::save(&path, 1, &BTreeMap::from([("0:0:HDMI".into(), 0)])).unwrap();
        let data = Settings::load(&path).unwrap();
        assert_eq!(data.last_gpu, 1);
        assert_eq!(data.gpus["0"]["0:5:DisplayPort"], 1023);
        assert_eq!(data.gpus["1"]["0:0:HDMI"], 0);
    }
    #[test]
    fn corrupt_or_invalid_file_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for text in [
            "{broken",
            r#"{"version":1,"last_gpu":0,"gpus":{"0":{"0:0:HDMI":1024}}}"#,
            r#"{"version":1,"last_gpu":0,"gpus":{"0":{"0:0:HDMI":true}}}"#,
        ] {
            fs::write(&path, text).unwrap();
            assert!(Settings::save(&path, 0, &BTreeMap::new()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
    }
    #[test]
    fn missing_file_is_read_only_and_bad_updates_fail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing/settings.json");
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
        assert!(!path.parent().unwrap().exists());
        assert!(Settings::save(&path, 16, &BTreeMap::new()).is_err());
        assert!(Settings::save(&path, 0, &BTreeMap::from([("invalid".into(), 0)])).is_err());
    }
    #[test]
    fn concurrent_saves_merge_gpu_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let threads: Vec<_> = (0..8)
            .map(|gpu| {
                let path = path.clone();
                std::thread::spawn(move || {
                    Settings::save(
                        &path,
                        gpu,
                        &BTreeMap::from([("0:0:HDMI".into(), i32::from(gpu))]),
                    )
                    .unwrap()
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(Settings::load(&path).unwrap().gpus.len(), 8);
    }
}
