# Contributing to HalluMeter

Thanks for your interest. HalluMeter is a small indie project — contributions are welcome but please read this before opening a PR.

---

## Before you start

- Open an issue first for anything non-trivial. Discuss the idea before writing code.
- Check existing issues and PRs — your idea may already be in progress.
- Small fixes (typos, docs, obvious bugs) can go straight to a PR.

---

## Setup

**Requirements:** Rust (stable), Node 24+

```bash
git clone https://github.com/Efs-O/hallumeter.git
cd hallumeter
npm install
npm run tauri dev
```

**Linux only** — install system deps first:
```bash
sudo apt-get install -y libasound2-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

---

## Before submitting a PR

All of these must pass locally:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npx vitest run
```

CI will reject the PR if any of these fail.

---

## Code rules

- Rust logic lives in `src-tauri/src/` — no business logic in the frontend
- Svelte 5 runes only (`$state`, `$derived`, `$effect`) — no Options API
- No new dependencies without discussion first
- Max 350 lines per source file
- All Rust paths via Tauri path API — no hardcoded OS paths

---

## Degradation curves

The data in `src-tauri/assets/curves.json` is research-backed. Do not change curve values without linking to a primary source in the PR description. See [RESEARCH.md](RESEARCH.md) for the existing sources.

---

## License

By contributing you agree your code will be released under the [Apache 2.0 license](LICENSE).
