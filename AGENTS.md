# Native App Agent Guide

This guide applies to the repository root and the Tauri native wrapper. The SPA in `SportsScoreboard/` is a separate Git submodule with its own `AGENTS.md`; follow that guide for SPA-specific work.

## Project Boundaries

- This repository owns the Tauri 2 shell, native Rust code, Windows and Android packaging, and release automation. The app embeds the built SPA in a native WebView; it is not a separate backend or hosted service.
- `src-tauri/src/` contains the Rust command and native startup. `src-tauri/tauri.conf.json` owns app identity, window settings, CSP, and the SPA build hooks.
- `SportsScoreboard/` owns the Vue UI, TypeScript application, ESPN adapter, domain logic, plugins, and frontend tests. Its changes belong to the submodule's repository; preserve the pinned submodule state and any existing edits.
- `scripts/` contains wrapper release-version tooling. `.github/workflows/` builds Windows and Android artifacts and handles SPA-release dispatches.

## Native ESPN Command

- `fetch_espn` is the only application-specific Tauri command. It is a constrained JSON transport for supported ESPN Site API routes, not a general-purpose HTTP proxy.
- Preserve the explicit URL allowlist in `src-tauri/src/espn_proxy.rs`: HTTPS, the exact ESPN API host, supported sport/league pairs, approved paths and query parameters, and numeric identifiers.
- Keep redirect validation, the request timeout, URL size limit, and response-size limit. Do not broaden hosts, routes, or parameters without a concrete product requirement and corresponding allowlist tests.
- Keep native capabilities minimal. Do not add filesystem, shell, or other permissions unless a requested feature requires them.
- Add Rust tests for allowlist, redirect, response-size, and error behavior when changing the proxy. Never weaken existing negative tests to make a new URL pass.

## Development And Validation

Install the wrapper dependencies with `npm ci`. For desktop development, `npm run dev` starts the SPA Vite server through Tauri and opens the native WebView. `npm run check` reports local Tauri tooling information.

Before completing native changes, run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

`npm run build` runs the Rust tests through its `prebuild` lifecycle hook, then builds the Tauri app. Its Tauri build hook also runs `npm run build` in `SportsScoreboard/`, which performs strict TypeScript checking, tests, and the SPA production build. The SPA's default Vitest discovery includes the live ESPN smoke test and therefore requires network access; for deterministic SPA unit and fixture integration tests, run from `SportsScoreboard/`:

```powershell
npx vitest run tests/unit tests/integration
```

For Android, use `npm run android:dev` or `npm run android:build -- --apk --target aarch64 --ci`. Android builds require the SDK, NDK, Java, and Rust target described in the root README.

Do not edit generated SPA output (`SportsScoreboard/dist/`), Rust build output (`src-tauri/target/`), or generated Android build files as source changes. Make changes in their owning source/configuration files and rebuild.

## Versions And Releases

- Keep the wrapper versions in the root `package.json` and `src-tauri/Cargo.toml` synchronized when intentionally changing the wrapper version.
- Release builds derive a composite app version from a stable SPA release and the wrapper version. Follow `.github/workflows/build.yml` and the release scripts; do not commit generated release overrides.
- Android release signing material is provided through GitHub Actions secrets. Never commit keystores, passwords, or generated signing configuration.

## Working Practices

- Inspect the relevant code, tests, and current worktree before editing. Preserve user changes and keep edits within the native-wrapper scope unless the task explicitly includes the SPA.
- Prefer focused Rust tests and validation over unrelated refactors. Keep the ESPN allowlist, Tauri capability boundary, and platform packaging behavior covered.
- Update the root README when native setup, supported targets, commands, permissions, or release behavior changes. Update `SportsScoreboard/README.md` and its `AGENTS.md` for SPA-specific behavior instead.
