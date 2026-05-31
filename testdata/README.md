# Test fixtures

Synthetic EXRs for exercising multichannel selection and optional alpha.
Generated procedurally (128×128, ZIP-compressed float).

- **aov_multilayer.exr** — a multilayer render: an HDR beauty pass with a
  hot highlight (`R`/`G`/`B`, values > 1.0) plus a radial `A`, and AOV
  layers `diffuse.*`, `specular.*`, normals `N.*`, and a single-channel
  depth `Z`. Exercises layer presets, per-channel R/G/B/A remapping, and
  alpha present.
- **beauty_rgb.exr** — plain RGB, no alpha. Exercises the "alpha is
  optional" path: the A view is disabled and the probe/overlay hide alpha.
- **beauty_rgba.exr** — RGBA with a real alpha gradient.
