# Pixelwise

A precision inspection and comparison tool for HDR images, render outputs, and ML-generated images. Runs entirely client-side — your images never leave the browser.

Sibling project to [Framewise](https://github.com/vork/Framewise), sharing its visual identity.

## Features

- Single-image deep inspection: exposure, tone mapping (Linear/Reinhard/ACES/Filmic/Gamma), channel views, false color, clipping overlays, NaN/Inf/negative detection, histogram with percentile markers, pixel probe.
- Side-by-side comparison: split slider, A/B flicker, difference / signed / log diff, onion-skin blend, synchronized pan/zoom and probe.
- Error metrics: MAE, MSE, RMSE, PSNR, SSIM, MS-SSIM, ΔE2000, relative & log-luminance error, error heatmaps, scatter plots.
- HDR-aware sanity checks: NaN/Inf/negative counts, EV diff, percentile diffs, dynamic-range estimate, color-space warnings.
- Deep zoom showing each pixel's actual float values overlaid.
- Native HDR display on supporting screens (WebGPU `rgba16float` + `display-p3` + extended-range tone mapping); SDR tone-mapped fallback otherwise.
- Drag-and-drop loading: EXR, Radiance HDR, PNG (8/16-bit), TIFF (float32 / 16-bit int).

## Tech stack

- **Rust → WebAssembly** with the **Leptos** UI framework.
- **wgpu** with the **WebGPU** backend for all rendering and image processing.
- **Tailwind CSS** for styling.
- **Trunk** for builds — output is a fully static bundle.

## Develop

```sh
# One-time
rustup target add wasm32-unknown-unknown
cargo install trunk wasm-bindgen-cli

# Run
trunk serve --open
```

## Build

```sh
trunk build --release
# `dist/` is the deployable static bundle.
```

## Browser support

Pixelwise requires **WebGPU**. As of 2026 this means Chrome/Edge 129+, Safari 26+, Firefox 141+ (best-effort). HDR display output additionally requires:
- An HDR-capable display.
- A browser/OS combination that exposes the HDR canvas (`rgba16float` + `display-p3` + `toneMapping: "extended"`).

Pixelwise auto-detects HDR capability and shows the current state in the title bar (`HDR P3 ACTIVE` / `SDR FALLBACK`).

## License

MIT.
