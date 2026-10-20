# Changelog

All notable changes to the craft-libs workspace are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the workspace follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every crate in the workspace shares one version (`[workspace.package] version` in `Cargo.toml`)
and the internal pins in `[workspace.dependencies]` match it, so a tag `vX.Y.Z` names one
consistent set of crates. Apps pin a tag or commit by git. While the version is `0.y.z`, a minor
bump (`0.1` to `0.2`) may break the public API of any crate; a patch bump does not.

## How to release

1. Move the entries under **Unreleased** into a new `## [X.Y.Z] - YYYY-MM-DD` section, set
   `[workspace.package] version` and every internal pin to `X.Y.Z`, and merge that to `main`.
   `scripts/check-version.sh X.Y.Z` checks both.
2. Run the **Tag release** workflow (`.github/workflows/tag.yml`) from the Actions tab with
   `X.Y.Z`. It checks the same things again on `main` and pushes the annotated tag `vX.Y.Z`.

## [Unreleased]

## [0.1.0] - 2026-10-20

First tagged release. Every crate starts at 0.1.0.

### Added

- `craft-ai-core`: the data model and contract for AI in the Craft apps (connections, models,
  tools, requests, jobs, consent, history, route spec), with `SecretRef` locators and
  `SecretString` values that cannot be printed or serialized.
- `craft-atomic`: crash-safe atomic file writing and replacement with same-directory staging,
  directory fsync, the Apple `F_FULLFSYNC` fallback, transient lock backoff and fault injection.
- `craft-cms`: pure-Rust ICC v2/v4 colour management, including the built-in Coated FOGRA39
  profile, soft proofing, gamut checks and 3D LUTs.
- `craft-docpath`: decides whether a path or `file:` URL stored in a document may be opened.
- `craft-i18n`: translations over the embedded catalogs, with fallback, plurals, language names,
  the system locale and a pseudo locale.
- `craft-pdf`: optional shared PDF reading, rasterization and guarded writing.
- `craft-print`: PDF submission to CUPS and Windows, printer discovery and driver preferences.
- `craft-quit`: native macOS quit, egui close handling and an optional app menu.
- `craft-settings`: `config.toml` storage with platform folders, portable mode, atomic saves,
  backup, damaged-file recovery and schema migrations.
- `craft-tokens`: the house design tokens generated from `tokens.json`, with egui conversions.
- `craft-ui`: SVG catalogs, icon buttons, tabs, pane controls, transactional docking, print
  settings, theme helpers and the `house` theme and components.
- `craft-undo`: the shared undo/redo stack with a command trait, coalescing, menu labels, a
  depth limit, the clean-state marker and a revision counter for autosave.
- Release tooling: this changelog, `scripts/check-version.sh` and the **Tag release** workflow.
