// SPDX-License-Identifier: GPL-3.0-only
//! Bounded child processes isolate potentially blocking NVIDIA driver calls.
use crate::{
    backend::{Discovery, Port},
    settings::{self, Settings},
};
use anyhow::{Context, Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub trait Hardware {
    fn discover(&self, gpu: u8) -> Result<Vec<Port>>;
    fn apply(&self, gpu: u8, values: &BTreeMap<String, i32>) -> Result<()>;
}
pub struct Native;
fn run(args: &[String]) -> Result<String> {
    let stdout = tempfile::tempfile()?;
    let stderr = tempfile::tempfile()?;
    let mut child = Command::new(std::env::current_exe()?)
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            bail!("NVIDIA request timed out after 15 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    };
    use std::io::{Read, Seek, SeekFrom};
    let mut stdout = stdout;
    let mut stderr = stderr;
    stdout.seek(SeekFrom::Start(0))?;
    stderr.seek(SeekFrom::Start(0))?;
    let mut output = String::new();
    stdout.read_to_string(&mut output)?;
    let mut error = String::new();
    stderr.read_to_string(&mut error)?;
    if !status.success() {
        bail!(
            "{}",
            if error.trim().is_empty() {
                output.trim()
            } else {
                error.trim()
            }
        );
    }
    Ok(output)
}
impl Hardware for Native {
    fn discover(&self, gpu: u8) -> Result<Vec<Port>> {
        Ok(serde_json::from_str::<Discovery>(&run(&[
            "--list-json".into(),
            "--gpu".into(),
            gpu.to_string(),
        ])?)?
        .ports)
    }
    fn apply(&self, gpu: u8, values: &BTreeMap<String, i32>) -> Result<()> {
        run(&[
            "--apply-json".into(),
            serde_json::to_string(values)?,
            "--gpu".into(),
            gpu.to_string(),
        ])?;
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Display {
    pub gpu: u8,
    pub port: Port,
}
pub fn discover_connected(hardware: &impl Hardware, gpus: &[u8]) -> Result<Vec<Display>> {
    let mut displays = Vec::new();
    for &gpu in gpus {
        let ports = hardware
            .discover(gpu)
            .with_context(|| format!("Cannot discover NVIDIA GPU {gpu}"))?;
        displays.extend(
            ports
                .into_iter()
                .filter(|port| port.connected)
                .map(|port| Display { gpu, port }),
        );
    }
    Ok(displays)
}

pub struct ApplyOutcome {
    pub saved: BTreeMap<u8, BTreeMap<String, i32>>,
    pub errors: Vec<String>,
}
pub fn apply_displays(
    hardware: &impl Hardware,
    path: &std::path::Path,
    values: &BTreeMap<u8, BTreeMap<String, i32>>,
) -> Result<ApplyOutcome> {
    for (&gpu, values) in values {
        anyhow::ensure!(gpu <= 15, "Invalid GPU index");
        settings::validate_values(values)?;
    }
    let mut outcome = ApplyOutcome {
        saved: BTreeMap::new(),
        errors: vec![],
    };
    for (&gpu, values) in values {
        match apply_and_save(hardware, path, gpu, values) {
            Ok(None) => {
                outcome.saved.insert(gpu, values.clone());
            }
            Ok(Some(error)) => outcome.errors.push(format!(
                "GPU {gpu}: Colours applied, but settings could not be saved: {error}"
            )),
            Err(error) => outcome
                .errors
                .push(format!("GPU {gpu}: Could not apply colours: {error:#}")),
        }
    }
    Ok(outcome)
}

/// Hardware failures must never save; a save failure is reported separately
/// because the display has already accepted the requested colours.
pub fn apply_and_save(
    hardware: &impl Hardware,
    path: &std::path::Path,
    gpu: u8,
    values: &BTreeMap<String, i32>,
) -> Result<Option<String>> {
    hardware.apply(gpu, values)?;
    Ok(Settings::save(path, gpu, values)
        .err()
        .map(|error| format!("{error:#}")))
}

pub fn restore(
    hardware: &impl Hardware,
    settings: &Settings,
    attempts: usize,
    mut pause: impl FnMut(),
) -> Result<()> {
    let mut pending: BTreeSet<u8> = settings
        .gpus
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(g, _)| g.parse().unwrap())
        .collect();
    let mut errors = BTreeMap::new();
    for attempt in 0..attempts {
        for gpu in pending.clone() {
            let result = (|| {
                let ports = hardware.discover(gpu)?;
                let values = ports
                    .iter()
                    .filter(|p| p.connected)
                    .filter_map(|p| settings.value(gpu, p).map(|v| (p.key(), v)))
                    .collect::<BTreeMap<_, _>>();
                if values.is_empty() {
                    bail!("No connected outputs with saved colours");
                }
                hardware.apply(gpu, &values)
            })();
            match result {
                Ok(()) => {
                    pending.remove(&gpu);
                }
                Err(error) => {
                    errors.insert(gpu, format!("{error:#}"));
                }
            }
        }
        if pending.is_empty() {
            return Ok(());
        }
        if attempt + 1 < attempts {
            pause();
        }
    }
    bail!(
        "{}",
        pending
            .iter()
            .map(|gpu| format!(
                "GPU {gpu}: {}",
                errors
                    .get(gpu)
                    .map(String::as_str)
                    .unwrap_or("Restore not attempted")
            ))
            .collect::<Vec<_>>()
            .join("; ")
    )
}
pub fn restore_login() -> Result<()> {
    let path = settings::path();
    let data = Settings::load(&path)?;
    // Serialize duplicate login/manual restore invocations.
    if data.gpus.values().all(|values| values.is_empty()) {
        return Ok(());
    }
    fs::create_dir_all(path.parent().context("Missing config directory")?)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_file_name(".restore.lock"))?;
    use fs2::FileExt;
    if lock.try_lock_exclusive().is_err() {
        return Ok(());
    }
    restore(&Native, &data, 12, || thread::sleep(Duration::from_secs(5)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    struct Fake {
        ready: RefCell<usize>,
        calls: RefCell<Vec<(u8, BTreeMap<String, i32>)>>,
    }
    impl Hardware for Fake {
        fn discover(&self, _: u8) -> Result<Vec<Port>> {
            let mut ready = self.ready.borrow_mut();
            if *ready > 0 {
                *ready -= 1;
                bail!("not ready");
            }
            Ok(crate::backend::demo_ports())
        }
        fn apply(&self, gpu: u8, values: &BTreeMap<String, i32>) -> Result<()> {
            self.calls.borrow_mut().push((gpu, values.clone()));
            Ok(())
        }
    }
    #[test]
    fn restore_retries_and_leaves_unsaved_outputs_untouched() {
        let fake = Fake {
            ready: RefCell::new(1),
            calls: RefCell::new(vec![]),
        };
        let settings = Settings {
            gpus: BTreeMap::from([("0".into(), BTreeMap::from([("0:0:HDMI".into(), 0)]))]),
            ..Settings::default()
        };
        let mut pauses = 0;
        restore(&fake, &settings, 3, || pauses += 1).unwrap();
        assert_eq!(pauses, 1);
        assert_eq!(
            *fake.calls.borrow(),
            vec![(0, BTreeMap::from([("0:0:HDMI".into(), 0)]))]
        );
    }
    #[test]
    fn missing_hardware_is_bounded_and_empty_settings_do_nothing() {
        let fake = Fake {
            ready: RefCell::new(20),
            calls: RefCell::new(vec![]),
        };
        restore(&fake, &Settings::default(), 3, || panic!("must not wait")).unwrap();
        let settings = Settings {
            gpus: BTreeMap::from([("0".into(), BTreeMap::from([("0:0:HDMI".into(), 512)]))]),
            ..Settings::default()
        };
        let mut pauses = 0;
        assert!(restore(&fake, &settings, 3, || pauses += 1).is_err());
        assert_eq!(pauses, 2);
        assert!(fake.calls.borrow().is_empty());
    }
    #[test]
    fn auto_detection_keeps_identical_connectors_on_different_gpus_separate() {
        struct Mixed;
        impl Hardware for Mixed {
            fn discover(&self, gpu: u8) -> Result<Vec<Port>> {
                let mut ports = crate::backend::demo_ports();
                ports[1].connected = gpu != 3;
                Ok(ports)
            }
            fn apply(&self, _: u8, _: &BTreeMap<String, i32>) -> Result<()> {
                unreachable!()
            }
        }
        let displays = discover_connected(&Mixed, &[0, 3]).unwrap();
        assert_eq!(displays.len(), 3);
        assert_eq!(
            displays.iter().map(|d| d.gpu).collect::<Vec<_>>(),
            vec![0, 0, 3]
        );
        assert_eq!(displays[0].port.key(), displays[2].port.key());
    }
    #[test]
    fn failed_gpu_never_saves_but_other_gpu_can_apply() {
        struct Partial;
        impl Hardware for Partial {
            fn discover(&self, _: u8) -> Result<Vec<Port>> {
                unreachable!()
            }
            fn apply(&self, gpu: u8, _: &BTreeMap<String, i32>) -> Result<()> {
                if gpu == 1 {
                    bail!("driver rejected value");
                }
                Ok(())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let values = BTreeMap::from([("0:0:HDMI".into(), 512)]);
        let result = apply_displays(
            &Partial,
            &path,
            &BTreeMap::from([(0, values.clone()), (1, values)]),
        )
        .unwrap();
        assert_eq!(result.saved.len(), 1);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("GPU 1"));
        let saved = Settings::load(&path).unwrap();
        assert!(saved.gpus.contains_key("0"));
        assert!(!saved.gpus.contains_key("1"));
    }
    #[test]
    fn save_error_is_distinct_from_driver_error_and_keeps_corrupt_file() {
        let fake = Fake {
            ready: RefCell::new(0),
            calls: RefCell::new(vec![]),
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{broken").unwrap();
        let values = BTreeMap::from([("0:0:HDMI".into(), 512)]);
        let result = apply_displays(&fake, &path, &BTreeMap::from([(0, values)])).unwrap();
        assert_eq!(fake.calls.borrow().len(), 1);
        assert!(result.saved.is_empty());
        assert!(result.errors[0].contains("Colours applied, but settings could not be saved"));
        assert_eq!(fs::read_to_string(path).unwrap(), "{broken");
    }
}
