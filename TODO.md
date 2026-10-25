# TODO
Just stuff to remember.

## Windows app signing
Users have to click the "Run anyways" option on Windows which doesn't inspire confidence.

## Windows support
The README says Owl runs on Windows, but locating `openmw.cfg` automatically is Linux-only (`crates/core/src/locate.rs` checks `--cfg`, XDG and Flatpak). Add `Documents\My Games\OpenMW` (and macOS `~/Library/Preferences/openmw`), then test the app end to end on Windows. Until then Windows users cannot rely on Owl finding their config.
