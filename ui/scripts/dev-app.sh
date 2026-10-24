#!/bin/sh
# Starts the app for development: `npm run dev:app`.
#
# Inside a container (distrobox, toolbox) the session bus points at the
# container's own bus, where the desktop portal cannot start, so the folder
# chooser does nothing. The host's bus is mounted at /run/host; use it.
# Elsewhere this changes nothing.
host_bus="/run/host/run/user/$(id -u)/bus"
if [ -e /run/.containerenv ] && [ -S "$host_bus" ]; then
  export DBUS_SESSION_BUS_ADDRESS="unix:path=$host_bus"
fi
exec npm run tauri dev -- "$@"
