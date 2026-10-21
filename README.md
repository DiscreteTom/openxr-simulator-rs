# OpenXR Simulator (Rust)

A desktop OpenXR runtime for Windows that lets OpenXR applications run in a
normal window without a headset. It simulates head tracking with the mouse and
keyboard so VR and XR applications can be developed and debugged on a PC.

The current backend is **D3D11**. The runtime shares the application's D3D11
device through `XR_KHR_D3D11_enable`, creates stereo swapchains that the
application can render into, and copies the left and right eye images to a
desktop window during `xrEndFrame`.

## Features

- OpenXR loader negotiation and a real `xrGetInstanceProcAddr` dispatch table
- Core Instance, System, Session, Reference Space and Action lifecycles
- D3D11 graphics binding and sessions
- Runtime-owned swapchains, including left/right slices with `arraySize = 2`
- GPU preview rendering directly from the application's D3D11 eye textures
- Desktop preview modes: stereo side-by-side, left eye only and right eye only
- FPS-style mouse look
- WASD movement, Q/E down/up and Shift speed boost
- Automatic mouse capture and release
- Persistent key bindings, mouse sensitivity, movement speed, FOV, IPD and render resolution
- Live FOV, IPD and eye-mode controls from the window menu
- Depth swapchain formats for `XR_KHR_composition_layer_depth`
- `XR_KHR_win32_convert_performance_counter_time`
- GitHub Actions tests, packaging and Windows releases for `v*` tags

## Requirements

- Windows 10/11 x64
- A D3D11-capable GPU
- An OpenXR loader supplied by the application, Unity, Unreal or the OpenXR SDK
- Rust stable when building from source

## Quick Start

Download and extract the release archive, then run:

```powershell
cd .\openxr-simulator-windows-x64
.\scripts\register-runtime.ps1
```

Start any D3D11 OpenXR application. The simulator window appears after the
application creates its Session.

To use the simulator for one process without changing the system runtime:

```powershell
.\scripts\run-with-simulator.ps1 `
  -Application "C:\Path\To\YourApp.exe"
```

Alternatively, set the environment variable directly:

```powershell
$env:XR_RUNTIME_JSON = "C:\Path\To\openxr-simulator-windows-x64\openxr_simulator.json"
```

Restore the previous runtime:

```powershell
.\scripts\unregister-runtime.ps1
```

The registration script backs up
`%LOCALAPPDATA%\openxr\1\active_runtime.json`. The unregister script restores
that backup when it exists.

## Controls

| Action | Default binding |
| --- | --- |
| Look around | Mouse |
| Move | W / S / A / D |
| Move up / down | E / Q |
| Move faster | Left Shift |
| Reset pose | R |
| Cycle eye view | F6 |
| Reload settings | F5 |
| Release mouse | Esc |
| Capture mouse | Click the preview window |

The **View** menu selects stereo, left-eye or right-eye preview. The
**FOV / IPD** menu adjusts horizontal FOV, vertical FOV and interpupillary
distance.

Mouse capture is active by default. Press Esc to release it before using the
menu with the mouse. Input is accepted only while a window from the current
process is in the foreground, so switching to a browser or editor does not
continue moving the simulated head.

## Configuration

The first launch creates:

```text
%LOCALAPPDATA%\OpenXR-Simulator-RS\settings.json
```

Set `OPENXR_SIMULATOR_CONFIG` to use a different file. The source example lives
at `packaging\settings.example.json` and is copied to the release archive as
`settings.example.json`:

```json
{
  "version": 1,
  "display": {
    "view_mode": "stereo",
    "window_width": 1280,
    "window_height": 720,
    "mirror_rate_hz": 90,
    "render_width": 1280,
    "render_height": 1408
  },
  "fov": {
    "horizontal_degrees": 100.0,
    "vertical_degrees": 96.0,
    "ipd_meters": 0.064
  },
  "movement": {
    "speed_meters_per_second": 3.0,
    "fast_multiplier": 4.0,
    "mouse_sensitivity_degrees_per_pixel": 0.12,
    "invert_y": false,
    "capture_mouse_on_start": true
  },
  "keys": {
    "forward": "W",
    "backward": "S",
    "strafe_left": "A",
    "strafe_right": "D",
    "up": "E",
    "down": "Q",
    "fast": "LShift",
    "reset_pose": "R",
    "cycle_eye": "F6",
    "reload_config": "F5"
  }
}
```

Key names are case-insensitive. Supported names include single letters and
digits, `Space`, `Tab`, `Escape`, arrow keys, `PageUp`, `PageDown`, `F1` through
`F24`, `LShift`/`RShift`, `LCtrl`/`RCtrl`, `LAlt`/`RAlt`, and
`MouseLeft`/`MouseRight`/`MouseMiddle`.

Press F5 after editing the settings. `render_width` and `render_height` are the
recommended per-eye render dimensions. Restart the OpenXR application after
changing them because it must recreate its swapchains.

## Build from Source

```powershell
cargo build --release --lib
```

The runtime is written to:

```text
target\release\openxr_simulator.dll
```

The version-controlled runtime manifest lives at
`packaging\openxr_simulator.json`. `package.ps1` copies it next to the DLL in
the release layout; the relative `library_path` is resolved from the directory
containing the manifest.

Source code is split by domain and every Rust file stays under 500 lines:

```text
src/runtime/
  instance.rs    Instance, System, view configuration and D3D11 requirements
  session.rs     Sessions, swapchains, frame loop and projection presentation
  presentation.rs
                 Projection-layer extraction, GPU preview and CPU fallback
  spaces.rs      Reference spaces, action spaces and view location
  actions.rs     Actions, paths, interaction profiles and synthetic input
  dispatch.rs    xrGetInstanceProcAddr dispatch table
src/d3d11_preview.rs
                 GPU swapchain preview with CPU readback fallback
src/window/
  mod.rs         Win32 lifecycle, mouse capture and frame publishing
  menu.rs        Menus and persisted settings commands
  message.rs     Window message handling
  paint.rs       GDI preview rendering
```

Create a local release package:

```powershell
.\scripts\package.ps1
```

This produces `dist\openxr-simulator-windows-x64.zip`.

## End-to-End Validation

The repository includes a probe that calls a real OpenXR loader. It creates a
D3D11 device, a Session and a stereo array swapchain, then submits five
projection frames:

```powershell
cargo build --release --example probe
Copy-Item .\packaging\openxr_simulator.json .\target\release\ -Force
$env:XR_RUNTIME_JSON = (Resolve-Path .\target\release\openxr_simulator.json).Path
$env:PATH = "C:\Program Files (x86)\Steam\steamapps\common\SteamVR\bin\win64;$env:PATH"
.\target\release\examples\probe.exe
```

The `PATH` example only locates `openxr_loader.dll`. SteamVR is not required;
any directory containing an OpenXR loader can be used.

## How It Works

1. The OpenXR loader loads `openxr_simulator.dll` and calls
   `xrNegotiateLoaderRuntimeInterface`.
2. The application provides its D3D11 device through
   `XR_KHR_D3D11_enable`.
3. The runtime creates swapchain textures with that device. The application
   renders each eye into an array slice.
4. During `xrEndFrame`, the runtime samples the current eye textures on the GPU
   and composites them into a D3D11 swapchain attached to the preview window.
5. Mouse movement updates head yaw and pitch; movement keys update head position.
6. `xrLocateViews` returns the configured eye poses and FOV.

Reference-space coordinates follow the OpenXR conventions:

- `STAGE` is floor-aligned; the simulated eye starts at `y = 1.7 m`
- `LOCAL` is anchored at the initial head pose, so the head starts at the origin
- `VIEW` is head-centered; per-eye poses contain only the half-IPD lateral offset

The primary preview path stays on the application's D3D11 queue and performs no
CPU pixel copies. A CPU readback path remains as a fallback when a GPU preview
swapchain cannot be created.

## Limitations

- Windows and D3D11 only; D3D12, Vulkan and OpenGL are not implemented
- The CPU fallback adds cost at 4K and other high render resolutions
- Only projection layers are presented; quad and cylinder overlays are not composited yet
- Controller emulation is minimal and does not include full input mapping, haptics or hand tracking
- No occlusion, environment understanding, foveated rendering or dynamic FOV
- Intended for development and debugging, not as a production headset runtime

## Logs and Troubleshooting

Logs are written to:

```text
%LOCALAPPDATA%\OpenXR-Simulator-RS\openxr_simulator.<pid>.log
```

Common checks:

- The application does not use the simulator: check `XR_RUNTIME_JSON` or
  `%LOCALAPPDATA%\openxr\1\active_runtime.json`
- The application uses D3D12 or Vulkan: switch it to D3D11
- No preview window: verify the application creates a D3D11 Session, not only an Instance
- The menu cannot be clicked: press Esc to release mouse capture
- Render resolution changes have no effect: close the application, edit the
  settings and restart it

## Roadmap

- D3D12 backend
- Vulkan backend
- Quad and UI composition layers
- Visual key-binding editor
- Headset profiles with asymmetric per-eye FOV
- Screenshot and recording support
- Automated pose scripts

## License

MIT
