# Third-party notices

## Intel Open Image Denoise 2.3.3

uHub 3D ships the Intel Open Image Denoise 2.3.3 prebuilt CPU
libraries (`OpenImageDenoise`, `OpenImageDenoise_core` and
`OpenImageDenoise_device_cpu`, with Intel oneTBB) to denoise path
traced thumbnails, as in Blender Cycles.

- Project: https://github.com/OpenImageDenoise/oidn
- License: Apache License 2.0 (see the `LICENSE.txt` shipped with
  the OIDN prebuilt distribution)
- The Rust `oidn` crate (MIT) provides the bindings used to call it.

Only the CPU device libraries are shipped. The CUDA, HIP, SYCL and
Metal device libraries are not included.
