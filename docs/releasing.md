# Releasing

CI (`.github/workflows/ci.yml`) runs on every push to `main` and on pull requests, on Linux and Windows: UI tests and build, `cargo fmt --check`, and the Rust tests.

## Cutting a release

1. Make sure `main` is committed, pushed and green in CI.
2. Run `scripts/release.sh 0.1.2` (the version, without a `v`). It sets the version in `Cargo.toml`, `ui/package.json` and `ui/src-tauri/tauri.conf.json` and in the two lock files, commits that as `chore: release 0.1.2`, and creates the tag `v0.1.2` on that commit. It refuses to run if you are not on `main`, have uncommitted changes, the tag exists, or the version is not newer. Add `--dry-run` to check without changing anything. It does not push.
3. Push the commit, then the tag: `git push origin main && git push origin v0.1.2`. Pushing the tag starts the release.
4. `.github/workflows/release.yml` checks the tag against those three versions, creates a **draft** release, and builds:
   - Linux: `.deb`, `.rpm` and `.AppImage`
   - Windows: an NSIS `.exe` installer
   - the `owl` command-line tool as `CLI_Owl_<version>_linux-x86_64.tar.gz` and `CLI_Owl_<version>_windows-x86_64.zip`
5. Open the draft on GitHub, check the assets and the generated notes, and publish it.

The Windows installer is not code-signed, so SmartScreen warns on first run.
