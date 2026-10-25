# Documentation

User-facing docs, one folder per product. Each product doc covers what it does, how to run it, and every environment variable and file location it reads or writes.

| Product | What it is | Docs |
|---|---|---|
| `owl` | Command-line tool | [owl-cli](owl-cli/README.md) |
| `owl-app` | Desktop app | [owl-app](owl-app/README.md) |
| `owl-core` | Rust library the CLI and the app are built on | [owl-core](owl-core/README.md) |

Why things are the way they are: [decisions/](decisions/0001-flatpak-distribution.md) (architecture decision records).

Releasing: [releasing.md](releasing.md). Project background: [research.md](research.md).

When a product gains a feature, flag, environment variable or file it touches, update its doc in the same change.
