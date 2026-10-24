# Owl

## Mission

Reduce the cognitive load and manual labor of playing modded Morrowind, so the game can reach more people more easily.

The measure of success is the player's experience, not the elegance of the tooling. Concretely:

- A new player goes from "I own Morrowind" to "I'm playing a good modded setup" in as few steps as possible, ideally one.
- The player never has to understand datapaths, load order, `openmw.cfg`, or which tool does what.
- We do not add a step to an existing workflow. Anything that does is the wrong design, however clean it is internally.
- Expert users can still inspect and override everything, but that is never required.

Every design decision should be checked against this. If a feature serves maintainers or tool authors but not the player, it's secondary.

See [docs/research.md](docs/research.md) for background.

## License

Copyright (C) 2026 Lucas Morgan

Licensed under GPL-3.0-or-later. See `LICENSE`. Issues are welcome; pull requests are not accepted (see `CONTRIBUTING.md`).
