# DevBench brand

Two circles overlap: the host and the guest. The overlap is where you work.

| File | Use |
|------|-----|
| `logo.svg` | Symbol in colour, transparent background |
| `logo-black.svg`, `logo-white.svg` | One-colour versions (the overlap is cut out) |
| `app-icon.svg` | App icon source; the Tauri icons in `src-tauri/icons/` are generated from it |

Regenerate the app icons after changing `app-icon.svg`: render it to a 1024×1024 PNG, then `npm run tauri icon <file>.png` (delete the generated `android/` and `ios/` folders, DevBench is desktop only).

| Colour | Hex | Role |
|--------|-----|------|
| Blue | `#2457e6` | host circle, primary accent |
| Turquoise | `#2dd4bf` | guest circle, secondary accent |
| Navy | `#0f1b3d` | overlap, dark text |
| Paper | `#f2f1ec` | icon background, light surface |
