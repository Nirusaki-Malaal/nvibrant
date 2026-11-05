---
title: Source Build
icon: material/git
description: Build the native Rust NVibrant application with Cargo and matching NVIDIA headers.
tags:
- Install
- Source
---

# Build from source

NVibrant is a native Rust + GTK 4 application. The previous Python/C++ build and
wheel packaging have been replaced by Cargo and a self-contained Rust executable.

```bash
git clone https://github.com/Nirusaki-Malaal/nvibrant.git
cd nvibrant
make deps
make check
make
make test
make install
```

Headers are selected for the running NVIDIA driver, or the pinned version when
building without a GPU. Override with `make DRIVER_VERSION=615.78.08` or an absolute
`NVIBRANT_NVIDIA_HEADERS` checkout. Native x86-64 and ARM64 builds are supported.

The installed app is independent of the source checkout. See
[BUILDING.md](https://github.com/Nirusaki-Malaal/nvibrant/blob/main/BUILDING.md)
for requirements, CLI commands, verification, migration and uninstall.
