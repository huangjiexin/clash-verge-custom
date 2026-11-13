# Custom subscription-group build

This private customization is based on Clash Verge Rev v2.5.10, upstream commit
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
