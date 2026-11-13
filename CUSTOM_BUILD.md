# Custom subscription-group build

This customization is based on Clash Verge Rev v2.5.10, upstream commit
`c83f7ad`. Upstream Git history is retained so the extension can be rebased.

The Subscriptions page includes independent subscription groups with selectable
sources, per-group scripts, concurrent connectivity checks, readable node labels,
result details and generic templates. See [the feature guide](docs/features/subscription-groups.md).

No personal subscriptions, tokens, node credentials or user settings are included.
Connectivity checks do not prove authenticated model-stream reliability. The
experimental repeated-check implementation discussed during diagnostics is not
included in this snapshot.

## Building

Use the Rust version from `rust-toolchain.toml` and Node with pnpm. Install the
platform dependencies listed in `CONTRIBUTING.md`, then run:

```sh
pnpm install --frozen-lockfile
pnpm prebuild <target>
pnpm exec tauri build --target <target> --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

Targets for this customization:

- `aarch64-apple-darwin`: Apple Silicon macOS, `.app` / `.dmg`.
- `x86_64-pc-windows-msvc`: Intel/AMD 64-bit Windows, NSIS installer.

The custom packages are not vendor-signed or notarized. The app identifier is
unchanged, so installation replaces the original app and uses its existing local
settings. Back up settings before replacing an installation. Upstream updater
endpoints remain unchanged; installing an official update can replace custom
features.

## Updating from upstream

```sh
git remote add upstream https://github.com/clash-verge-rev/clash-verge-rev.git
git fetch upstream
git rebase <reviewed-upstream-ref>
```

Resolve the small integration hooks described in the feature guide, regenerate
IPC contracts, and validate frontend and native builds before creating packages.

The manual **Custom Windows package** GitHub Actions workflow builds an unsigned
Windows x64 installer and retains it as a workflow artifact for 30 days.
It does not create a release, publish an updater manifest, or deploy the app.

### Cross-compiling Windows from macOS

The 2026-11-13 package was also compiled with `cargo-xwin 0.23.1`, LLVM/LLD 21,
and the Microsoft CRT/SDK. Use `cargo-xwin` as Tauri's runner with the Windows
target. Prepare Windows resources in a separate source directory so they do not
mix with macOS service binaries. Keep the downloaded SimpleSC plugin available to
NSIS in addition to Tauri's own NSIS plugin. When invoking macOS `makensis`, set
`LC_ALL=en_US.UTF-8 LANG=en_US.UTF-8`; the host's unset locale caused NSIS to abort
while processing `StrFunc.nsh`. The x64 application and embedded services still
require Windows runtime validation; cross-compilation is not a Windows smoke test.
