# Releasing

CI (`.github/workflows/ci.yml`) runs on every push to `main` and on pull requests, on Linux and Windows: UI tests and build, `cargo fmt --check`, clippy with warnings denied, and the Rust tests.

## Cutting a release

1. Set the same version in `Cargo.toml` (`[workspace.package]`), `ui/src-tauri/tauri.conf.json` and `ui/package.json`.
2. Commit, then tag and push: `git tag v0.1.0 && git push origin v0.1.0`.
3. `.github/workflows/release.yml` checks the tag against those three versions, creates a **draft** release, and builds:
   - Linux: `.deb`, `.rpm` and `.AppImage`
   - Windows: an NSIS `.exe` installer
   - the `owl` command-line tool as `CLI_Owl_<version>_linux-x86_64.tar.gz` and `CLI_Owl_<version>_windows-x86_64.zip`
4. Open the draft on GitHub, check the assets and the generated notes, and publish it.

The Windows installer is not code-signed, so SmartScreen warns on first run.
