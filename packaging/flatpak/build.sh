#!/bin/sh
# Builds ./owl.flatpak from the release binaries. Needs flatpak-builder (the org.flatpak.Builder Flatpak works) and the
# GNOME 50 runtime and SDK (flatpak install flathub org.gnome.Platform//50 org.gnome.Sdk//50).
set -e
cd "$(dirname "$0")"
flatpak run org.flatpak.Builder --force-clean --user --repo=repo build-dir app.owl.openmw.yml
flatpak build-bundle repo owl.flatpak app.owl.openmw
echo "Install with: flatpak install --user owl.flatpak"
