# GLB Studio

A native GLB/glTF desktop viewer built with **Rust, miniquad, and macroquad**.
It runs on Windows, macOS, and Linux. Models are processed locally; the app does
not need a server, browser, or internet connection.

## Download builds

Open a successful GitHub Actions run and download its artifact:

| Platform | Workflow | Artifact | Distribution ZIP |
| --- | --- | --- | --- |
| Windows x64 | [Build Windows app](https://github.com/rmashukov/glbstudio/actions/workflows/windows-build.yml) | `GLB-Studio-Windows-x64` | `glb-studio-windows-x64.zip` |
| macOS, Apple Silicon and Intel | [Build macOS app](https://github.com/rmashukov/glbstudio/actions/workflows/macos-build.yml) | `GLB-Studio-macOS-Universal` | `glb-studio-macos-universal.zip` |

Each artifact contains a distribution ZIP. Extract that ZIP, then open
`GLB Studio.exe` on Windows or `GLB Studio.app` on macOS. The macOS build requires
macOS 11 or later and uses an ad-hoc signature; it is not Developer ID signed
or notarized.

Both workflows run on pushes and can be started manually with **Run workflow**.
They build and package the app without launching it. Artifacts are retained for
30 days. GitHub requires sign-in to download Actions artifacts.

## Getting started

Start the app, choose **Open GLB**, drag a model into the window, or pass a model
path on the command line. Without a file, the app displays a built-in textured
robot with 11 joints and two animations.

To run from source:

```sh
cargo run --release
cargo run --release -- "path/to/model.glb"
```

The viewer opens `.glb` and `.gltf` files with local resources. File selection
uses a native dialog. Model parsing and texture decoding run on a worker thread.
Import errors appear in the app, and the previous model remains available.

## Controls

| Action | Control |
| --- | --- |
| Orbit around the focus point | Left mouse button + drag |
| Move the focus point in the screen plane | Right mouse button + drag |
| Zoom | Mouse wheel |
| Fit the current pose in the viewport | `F` / Fit |
| Reset the view and fit the model | `Home` |
| Open a model | `O`, `Ctrl+O` / Open GLB |
| View materials and lighting | `1` / Shaded |
| View the wireframe, including back faces | `2` / Wire |
| Overlay the wireframe on the shaded model | `3` / Overlay |
| Show the skeleton through the model | `S` / Skeleton x-ray |
| Toggle the ground grid | `G` / Ground grid |
| Play or pause animation | `Space` / Play / Pause |
| Previous or next animation | Left/right arrow keys or `<` / `>` buttons |
| Scrub the animation | Click or drag the timeline slider |

The animation list also includes **Rest pose**. Playback supports speeds from
0.25x to 2x, looping, and returning to the start of a clip. Scrubbing pauses
playback. Below 860 logical pixels of window width, the side panel is hidden;
keyboard shortcuts and the timeline remain available.

Each wheel notch changes camera distance by approximately 4%, with a smooth
transition of about 0.25 seconds. Scroll units are normalized for Windows,
macOS, and X11, preserving fractional trackpad movement. `F` and `Home` reset
any pending zoom transition.

## Rendering and format support

- Base color, vertex colors, metallic/roughness, normal, occlusion, and emissive
  maps; PNG and JPEG textures.
- Three studio lights, hemispherical fill lighting, exposure control,
  linear/sRGB conversion, and an ACES tone-mapping approximation.
- Alpha MASK/BLEND, double-sided materials, and `KHR_materials_unlit`.
- Node hierarchies, matrix transforms, indexed and non-indexed triangles,
  triangle strips/fans, and generated normals when they are missing.
- Skinning with up to eight influences per vertex (`JOINTS_0/1`, `WEIGHTS_0/1`),
  inverse bind matrices, and skeleton visualization in the current pose.
- Translation, rotation, scale, and morph-weight animation; LINEAR, STEP, and
  CUBICSPLINE interpolation; shortest-path slerp for linear quaternion animation.
- Persistent miniquad GPU buffers. Large meshes are split into batches with
  local 16-bit indices. Static meshes are not uploaded on every frame.

All desktop platforms use OpenGL. macOS uses an OpenGL 3.2 core context with
GLSL 1.50 shaders. Metal shaders are retained and checked during macOS builds,
but the Metal backend is not selected: miniquad 0.4.11 lacks the viewport,
line drawing, and MSAA resolve support needed by this viewer.

### Current limitations

Skinning runs on the CPU, so dense animated models may be slower than static
models. Normal maps reconstruct the tangent basis from derivatives. Real-time
shadows and HDR environment maps are not implemented. Transparent surfaces are
sorted by mesh-part centers, so intersecting transparent geometry may render
incorrectly.

The default scene is loaded, falling back to the first scene. Point and line
primitives are skipped. Each material uses one UV set; additional maps using a
different UV set are disabled with an import note. Draco, meshopt, KTX2/Basis,
texture transforms, and transmission/volume/clearcoat materials are not
implemented. Unsupported **required** extensions produce an error; optional
extensions are ignored with an import note. Image dimensions are limited to
16384 pixels and must also be supported by the GPU.

## Logs and diagnostics

A log file is created automatically **before the window is created**, including
when the app is launched from Finder:

| Platform | Log path |
| --- | --- |
| macOS | `~/Library/Logs/GLB Studio/glb-studio.log` |
| Windows | `%LOCALAPPDATA%\GLB Studio\Logs\glb-studio.log` |
| Linux | `${XDG_STATE_HOME:-~/.local/state}/glb-studio/glb-studio.log` |

On macOS, press `Cmd+Shift+G` in Finder and enter `~/Library/Logs/GLB Studio/`.
When reporting a blank window or a crash, attach `glb-studio.log` after
reproducing the issue. It records the version/build commit, OS and architecture,
graphics backend, initialization stages, model imports, initial frames, errors,
and panic backtraces. On macOS and Linux, stdout/stderr are redirected to the
same file, including native graphics-library messages. Model contents are not
written to the log.

Sessions are appended to the same file. If it exceeds 2 MiB at the next launch,
the previous log is saved as `glb-studio.previous.log`. Set `GLB_STUDIO_LOG_DIR`
to choose a different directory. If the normal directory is unavailable, the
app falls back to a `GLB Studio` directory inside the system temporary directory.

## Building from source

Install stable Rust with [rustup](https://rustup.rs/).

### Windows x64

Use the MSVC Rust toolchain and install Visual Studio Build Tools with the C++
workload and Windows SDK. Build and package without launching the app:

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --release --locked --bin glb-viewer --target x86_64-pc-windows-msvc
python tools/package.py --binary target/x86_64-pc-windows-msvc/release/glb-viewer.exe --arch x64
```

The result is `dist/glb-studio-windows-x64.zip`, containing `GLB Studio.exe`,
this README, and license files. The Windows workflow uses the same commands.

### macOS

For a build targeting the current Mac, install Xcode Command Line Tools with
`xcode-select --install`, then run:

```sh
cargo build --release --locked --bin glb-viewer
python3 tools/package.py
```

For a Universal build containing both Apple Silicon and Intel binaries, use
Xcode with the Metal compiler, stable Rust, Python 3.9 or later, and glslang:

```sh
brew install glslang
python3 tools/build_macos.py
```

The script checks GLSL 1.50 and Metal shaders without opening a graphics window,
builds both architectures, combines them with `lipo`, verifies the architectures,
and creates `dist/glb-studio-macos-universal.zip` containing `GLB Studio.app`.
It applies and verifies an ad-hoc signature without launching the app.
The macOS workflow runs the same script.

### Linux

Install a C toolchain, X11, and OpenGL/Mesa. On Debian or Ubuntu:

```sh
sudo apt install build-essential pkg-config libx11-dev libxi-dev libgl1-mesa-dev
cargo build --release --locked --bin glb-viewer
python3 tools/package.py
```

The native file dialog requires `xdg-desktop-portal` and a desktop-specific
backend, such as `xdg-desktop-portal-gtk` or the KDE backend. The app uses rfd's
portal backend; GTK development packages are not required to build the dialog.

## Development checks

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo run --release --bin glb-inspect -- "path/to/model.glb"
```

`glb-inspect` accepts multiple paths, checks imports, and evaluates every
animation at 0%, 25%, 50%, 75%, and 100% of its duration without creating a window.
It exits with a nonzero status on errors.

For an optional graphics smoke test, set `GLB_VIEWER_SMOKE_FRAMES=90` before
launching the app. It cycles through Shaded, Wire, and Overlay with the skeleton,
then closes the window. Build workflows do not run this test.

`python tools/make_demo.py` regenerates the original demo GLB model.
User test models are not included in the repository or distributions.

## Project layout

- `src/scene.rs`, `src/animation.rs`: CPU import, poses, skinning, and morphs.
- `src/render.rs`, `src/shaders/`: miniquad GPU buffers and GLSL/MSL shaders.
- `src/camera.rs`: orbit, pan, zoom, and skeleton projection.
- `src/ui.rs`, `src/player.rs`: macroquad UI and animation playback.
- `src/main.rs`: window, main loop, and background model loading.
- `src/diagnostics.rs`: file logging, startup diagnostics, and panic hook.
- `src/bin/glb-inspect.rs`: model validation without a window.
- `tools/package.py`: platform-specific distribution packaging.
- `tools/build_macos.py`: Universal macOS build and shader checks.
- `.github/workflows/`: Windows and macOS build workflows.

## Libraries and licenses

Library documentation: [macroquad](https://docs.rs/macroquad/0.4.16/macroquad/),
[miniquad](https://docs.rs/miniquad/0.4.11/miniquad/),
[gltf](https://docs.rs/gltf/1.4.1/gltf/), and [rfd](https://docs.rs/rfd/0.15.4/rfd/).

GLB Studio is licensed under the MIT License; see `LICENSE`.
Noto Sans uses the SIL Open Font License, included in `assets/OFL-NotoSans.txt`.
