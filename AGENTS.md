# Working Agreement

Short rules for working in this repository.

## 1. Commits

Format every message as `type(scope): subject`, for example
`fix(swapchain): preserve D3D11 image array stride`. Use the imperative mood and
keep the subject under ~72 characters. Common types: `feat`, `fix`, `refactor`,
`test`, `docs`, `chore`.

Never run `git commit`, `git push`, or another history-changing command unless
the user explicitly approves it. When approval is given, write the message to a
file and commit with `git commit -F <file>`, then read it back with
`git log -1 --format=%B` to catch shell-escaping mistakes.

## 2. Keep files small

Aim to keep every code file under 500 lines. When a file grows past that, split
it along business-logic boundaries, not merely by technical layer. An OpenXR
runtime naturally separates into instance/system discovery, session and frame
lifecycle, spaces/views, actions/paths, window lifecycle, menu behavior and
preview presentation.

## 3. Tests guard every change

Add a test for every bug fix or behaviour change that can be tested, to prevent
regressions. When testing is not practical, explain why in the final report.

Prefer behavioural tests. OpenXR C ABI bugs deserve direct ABI-shaped tests:
for example, verify that arrays of `XrSwapchainImageD3D11KHR` use their real
element stride instead of `sizeof(XrSwapchainImageBaseHeader)`.

## 4. Treat the OpenXR ABI as unsafe external input

- Preserve every pointer, array stride and output-buffer contract exactly.
- Never advance a base-structure pointer through an array whose concrete
  structure is larger.
- Respect `capacity == 0` sizing calls and return
  `XR_ERROR_SIZE_INSUFFICIENT` when an output buffer is too small.
- Treat fixed-size OpenXR character arrays as potentially unterminated.
- Keep all exported functions `extern "system"` and document their safety
  requirements.
- Do not change the user's active OpenXR runtime during automated tests. Use a
  temporary `LOCALAPPDATA` or `XR_RUNTIME_JSON` instead.

## 5. Test exclusive Windows/GPU resources carefully

Parallel tests can race while creating or tearing down D3D11, DXGI, Vulkan,
windows or audio devices. If a GPU test is flaky, reproduce it with the
narrowest filter first. Serialise only that suite, for example:

```powershell
cargo test d3d11::tests -- --test-threads=1
```

Do not globally force one test thread to hide a race. Isolate exclusive-resource
tests by process or a dedicated worker when possible.

## 6. Keep user-facing documentation in sync

Architecture, ABI behavior and user-visible defaults are documented in
[`README.md`](README.md). When a design decision changes, update the README in
the same change. Tunable defaults also belong in
[`settings.example.json`](packaging/settings.example.json).

## 7. Package the runtime when code changes

When a change can affect the runtime binary, run:

```powershell
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
.\scripts\package.ps1
```

Then run the end-to-end OpenXR loader probe against the packaged runtime:

```powershell
$env:XR_RUNTIME_JSON = (Resolve-Path .\dist\openxr-simulator-windows-x64\openxr_simulator.json).Path
.\target\release\examples\probe.exe
```

The probe must create a D3D11 session and submit projection frames. Report the
release ZIP path. Documentation-only changes do not require repackaging.
