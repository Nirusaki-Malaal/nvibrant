use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=NVIBRANT_NVIDIA_HEADERS");
    let headers =
        PathBuf::from(env::var_os("NVIBRANT_NVIDIA_HEADERS").unwrap_or_else(|| "open-gpu".into()));
    let version_file = headers.join("version.mk");
    println!("cargo:rerun-if-changed={}", version_file.display());
    let version_text =
        std::fs::read_to_string(&version_file).expect("NVIDIA header checkout needs version.mk");
    let version = version_text
        .lines()
        .find_map(|line| line.strip_prefix("NVIDIA_VERSION = "))
        .expect("Missing NVIDIA header version")
        .trim();
    println!("cargo:rustc-env=NVIBRANT_BUILD_DRIVER={version}");
    let mut builder = bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <sys/ioctl.h>\n#include \"nvkms-ioctl.h\"\n#include \"nvkms-api.h\"\nstatic const unsigned long NVIBRANT_IOCTL = NVKMS_IOCTL_IOWR;\n")
        .allowlist_type("NvKms(AllocDevice|FreeDevice|QueryDisp|QueryConnectorStaticData|QueryDpyDynamicData|SetDpyAttribute)(Params|Request|Reply)")
        .allowlist_type("NvKmsIoctlParams|NvKmsIoctlCommand")
        .allowlist_var("NVIBRANT_IOCTL|NVKMS_IOCTL_.*|NV_KMS_DPY_ATTRIBUTE_.*|NVKMS_CONNECTOR_TYPE_.*|NVKMS_ALLOC_DEVICE_STATUS_.*")
        .derive_default(true)
        .prepend_enum_name(false)
        .generate_comments(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));
    for directory in [
        "src/nvidia-modeset/interface",
        "src/nvidia-modeset/include",
        "src/common/inc",
        "src/common/sdk/nvidia/inc",
        "src/common/unix/common/inc",
    ] {
        builder = builder.clang_arg(format!("-I{}", headers.join(directory).display()));
    }
    let bindings = builder
        .generate()
        .expect("NVIDIA headers required; run ./build.sh build");
    println!("cargo:rustc-check-cfg=cfg(nvidia_struct_device_id)");
    if bindings.to_string().contains("pub deviceId: NvKmsDeviceId") {
        println!("cargo:rustc-cfg=nvidia_struct_device_id");
    }
    bindings
        .write_to_file(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("nvidia.rs"))
        .unwrap();
}
