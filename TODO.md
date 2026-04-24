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

### 3. Add CSP to Tauri config

Current state:
- `security.csp` is `null`

Proposed `src-tauri/tauri.conf.json` values:

```json
"security": {
  "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: data:; font-src 'self' data:; media-src 'self' asset: data:; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; frame-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
  "devCsp": "default-src 'self' http://localhost:1420 ws://localhost:1420; script-src 'self' 'unsafe-eval' http://localhost:1420; style-src 'self' 'unsafe-inline' http://localhost:1420; img-src 'self' asset: data: http://localhost:1420; font-src 'self' data: http://localhost:1420; media-src 'self' asset: data: http://localhost:1420; connect-src 'self' ipc: http://ipc.localhost http://localhost:1420 ws://localhost:1420; object-src 'none'; frame-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'"
}
```

Notes:
- Production CSP is intentionally strict.
- `style-src 'unsafe-inline'` is currently needed because the Svelte UI uses inline styles.
- `devCsp` stays looser to support Vite dev server and websocket/HMR behavior.
