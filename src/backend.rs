use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    os::fd::AsRawFd,
};

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    clippy::all
)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/nvidia.rs"));
}
use ffi::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Port {
    pub index: usize,
    pub display: u32,
    pub connector: u32,
    #[serde(rename = "type")]
    pub kind: String,
    pub connected: bool,
}
impl Port {
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.display, self.connector, self.kind)
    }
}
#[derive(Serialize, Deserialize)]
pub struct Discovery {
    pub ports: Vec<Port>,
}

// Only this module crosses the NVIDIA C ABI. All layouts come from the selected
// official driver headers; callers never construct raw pointers or ioctl values.
struct Device {
    file: File,
    handle: u32,
    displays: Vec<u32>,
}
fn ioctl<T>(file: &File, command: u32, data: &mut T) -> Result<()> {
    let mut params = NvKmsIoctlParams {
        cmd: command,
        size: std::mem::size_of::<T>() as u32,
        address: data as *mut T as u64,
    };
    // SAFETY: params and data are live, correctly aligned, writable C-layout
    // objects for this synchronous ioctl. The kernel does not retain the pointer.
    let result = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            NVIBRANT_IOCTL as libc::c_ulong,
            &mut params,
        )
    };
    if result < 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("NVIDIA ioctl {command} failed"));
    }
    Ok(())
}
impl Device {
    fn open(gpu: u8) -> Result<Self> {
        ensure!(gpu <= 15, "GPU index must be between 0 and 15");
        let version = env::var("NVIDIA_DRIVER_VERSION_CPP")
            .or_else(|_| fs::read_to_string("/sys/module/nvidia/version"))
            .context("NVIDIA driver not loaded; cannot read /sys/module/nvidia/version")?;
        let version = version.trim();
        ensure!(
            version == env!("NVIBRANT_BUILD_DRIVER"),
            "This build targets NVIDIA {}; the running driver reports {}. Rebuild and reinstall against matching headers.",
            env!("NVIBRANT_BUILD_DRIVER"),
            version
        );
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/nvidia-modeset")
            .context("Cannot open /dev/nvidia-modeset. Check NVIDIA modesetting and permissions")?;
        let mut data = NvKmsAllocDeviceParams::default();
        ensure!(
            version.len() < data.request.versionString.len() && !version.contains('\0'),
            "Invalid driver version"
        );
        // Select the physical GPU; leave the MIG partition at its default.
        data.request.deviceId.rmDeviceId = u32::from(gpu);
        for (slot, byte) in data.request.versionString.iter_mut().zip(version.bytes()) {
            *slot = byte as _;
        }
        data.request.no3d = 1;
        ioctl(&file, NVKMS_IOCTL_ALLOC_DEVICE, &mut data)
            .context("Cannot allocate NVIDIA GPU; headers must match the running driver")?;
        let count = data.reply.numDisps as usize;
        ensure!(
            count <= data.reply.dispHandles.len(),
            "Invalid display count from NVIDIA driver"
        );
        Ok(Self {
            file,
            handle: data.reply.deviceHandle,
            displays: data.reply.dispHandles[..count].to_vec(),
        })
    }
    fn ports(&self) -> Result<Vec<(Port, u32, NVDpyId)>> {
        let mut ports = Vec::new();
        for (display, &handle) in self.displays.iter().enumerate() {
            let mut query = NvKmsQueryDispParams::default();
            query.request.deviceHandle = self.handle;
            query.request.dispHandle = handle;
            ioctl(&self.file, NVKMS_IOCTL_QUERY_DISP, &mut query)?;
            let count = query.reply.numConnectors as usize;
            ensure!(
                count <= query.reply.connectorHandles.len(),
                "Invalid connector count"
            );
            for connector in 0..count {
                let mut data = NvKmsQueryConnectorStaticDataParams::default();
                data.request.deviceHandle = self.handle;
                data.request.dispHandle = handle;
                data.request.connectorHandle = query.reply.connectorHandles[connector];
                ioctl(
                    &self.file,
                    NVKMS_IOCTL_QUERY_CONNECTOR_STATIC_DATA,
                    &mut data,
                )?;
                let mut dynamic = NvKmsQueryDpyDynamicDataParams::default();
                dynamic.request.deviceHandle = self.handle;
                dynamic.request.dispHandle = handle;
                dynamic.request.dpyId = data.reply.dpyId;
                ioctl(&self.file, NVKMS_IOCTL_QUERY_DPY_DYNAMIC_DATA, &mut dynamic)?;
                let kind = match data.reply.type_ {
                    NVKMS_CONNECTOR_TYPE_HDMI => "HDMI",
                    NVKMS_CONNECTOR_TYPE_DP => "DisplayPort",
                    _ => "Display",
                };
                ports.push((
                    Port {
                        index: ports.len(),
                        display: display as u32,
                        connector: connector as u32,
                        kind: kind.into(),
                        connected: dynamic.reply.connected != 0,
                    },
                    handle,
                    data.reply.dpyId,
                ));
            }
        }
        Ok(ports)
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        let mut data = NvKmsFreeDeviceParams::default();
        data.request.deviceHandle = self.handle;
        if let Err(error) = ioctl(&self.file, NVKMS_IOCTL_FREE_DEVICE, &mut data) {
            eprintln!("{error:#}");
        }
    }
}
pub fn discover(gpu: u8) -> Result<Vec<Port>> {
    Ok(Device::open(gpu)?
        .ports()?
        .into_iter()
        .map(|(p, _, _)| p)
        .collect())
}

pub fn apply(gpu: u8, values: &BTreeMap<String, i32>, attribute: &str) -> Result<()> {
    let (attribute, min, max) = match attribute {
        "vibrance" => (NV_KMS_DPY_ATTRIBUTE_DIGITAL_VIBRANCE, -1024, 1023),
        "dithering" => (NV_KMS_DPY_ATTRIBUTE_REQUESTED_DITHERING, 0, 2),
        "dithering-mode" => (NV_KMS_DPY_ATTRIBUTE_REQUESTED_DITHERING_MODE, 0, 3),
        "dithering-depth" => (NV_KMS_DPY_ATTRIBUTE_REQUESTED_DITHERING_DEPTH, 0, 3),
        _ => bail!("Unknown attribute"),
    };
    ensure!(!values.is_empty(), "No connected saved outputs to apply");
    ensure!(
        values
            .iter()
            .all(|(k, v)| crate::settings::valid_key(k) && (min..=max).contains(v)),
        "Invalid output or attribute value"
    );
    let device = Device::open(gpu)?;
    let ports = device.ports()?;
    // Validate every target before changing any output; a refresh can race unplug.
    for key in values.keys() {
        ensure!(
            ports.iter().any(|(p, _, _)| p.connected && p.key() == *key),
            "Output {key} disconnected; refresh displays"
        );
    }
    for (port, display, id) in ports {
        if let Some(&value) = values.get(&port.key()).filter(|_| port.connected) {
            let mut data = NvKmsSetDpyAttributeParams::default();
            data.request.deviceHandle = device.handle;
            data.request.dispHandle = display;
            data.request.dpyId = id;
            data.request.attribute = attribute;
            data.request.value = i64::from(value);
            ioctl(&device.file, NVKMS_IOCTL_SET_DPY_ATTRIBUTE, &mut data)?;
        }
    }
    Ok(())
}

pub fn demo_ports() -> Vec<Port> {
    vec![
        Port {
            index: 0,
            display: 0,
            connector: 0,
            kind: "HDMI".into(),
            connected: true,
        },
        Port {
            index: 1,
            display: 0,
            connector: 1,
            kind: "DisplayPort".into(),
            connected: true,
        },
    ]
}

/// The driver's proc entries expose each GPU's RM device minor. Discover these
/// instead of asking the user for an index or probing nonexistent GPUs.
pub fn gpu_indices_at(root: &std::path::Path) -> Result<Vec<u8>> {
    let mut indices = std::collections::BTreeSet::new();
    for entry in fs::read_dir(root)
        .context("No NVIDIA GPUs detected; check that the NVIDIA driver is loaded")?
    {
        let information = fs::read_to_string(entry?.path().join("information"))?;
        if information.lines().any(|line| {
            line.starts_with("GPU Excluded:")
                && line
                    .split(':')
                    .nth(1)
                    .is_some_and(|value| value.trim() == "Yes")
        }) {
            continue;
        }
        let index: u8 = information
            .lines()
            .find_map(|line| line.strip_prefix("Device Minor:"))
            .context("NVIDIA GPU information has no device minor")?
            .trim()
            .parse()?;
        ensure!(index <= 15, "Unsupported NVIDIA device minor {index}");
        indices.insert(index);
    }
    ensure!(!indices.is_empty(), "No NVIDIA GPUs detected");
    Ok(indices.into_iter().collect())
}
pub fn gpu_indices() -> Result<Vec<u8>> {
    gpu_indices_at(std::path::Path::new("/proc/driver/nvidia/gpus"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn autodetects_sparse_gpu_minors_and_skips_excluded_devices() {
        let dir = tempfile::tempdir().unwrap();
        for (bus, text) in [
            ("0000:01:00.0", "Device Minor: 3\nGPU Excluded: No\n"),
            ("0000:02:00.0", "Device Minor: 0\nGPU Excluded: No\n"),
            ("0000:03:00.0", "Device Minor: 2\nGPU Excluded: Yes\n"),
        ] {
            fs::create_dir(dir.path().join(bus)).unwrap();
            fs::write(dir.path().join(bus).join("information"), text).unwrap();
        }
        assert_eq!(gpu_indices_at(dir.path()).unwrap(), vec![0, 3]);
    }
}
