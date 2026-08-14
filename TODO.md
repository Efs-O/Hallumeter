# TODO

## Pending config changes

### 1. Change Tauri bundle identifier

Update `src-tauri/tauri.conf.json`:

```json
"identifier": "com.efso.hallumeter"
```

### 2. Reduce CI packaging scope in GitHub Actions

Goal:
- Keep `cargo fmt`, `cargo clippy`, `cargo test`, and `vitest` on all OSes
- Stop running full `npm run tauri build` packaging on every matrix leg for normal push/PR CI
- Reserve full cross-platform packaging for release workflow or tags

Suggested direction:
- `build.yml`: run validation on `windows-latest`, `ubuntu-latest`, and `macos-latest`
- Run `npm run tauri build` on one platform only for regular CI, or gate packaging behind tags/releases
