# Run `just` to list the recipes.

default:
    @just --list

# Install the UI dependencies.
setup:
    cd ui && npm ci

# Start the app for development; extra arguments go to `tauri dev`.
run *args:
    sh ui/scripts/dev-app.sh {{args}}

# Build everything. The app crate embeds ui/dist, so the UI goes first.
build:
    cd ui && npm run build
    cargo build --workspace

# Run all tests, Rust and UI.
test:
    cd ui && npm test
    cargo test --workspace

# Prepare a release, e.g. `just release 0.1.2 --dry-run`.
release version *flags:
    scripts/release.sh {{version}} {{flags}}
