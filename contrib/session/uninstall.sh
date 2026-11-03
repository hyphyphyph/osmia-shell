#!/bin/sh
# Remove the Osmia login session and give SDDM back its old default.
set -eu
if [ "$(id -u)" -ne 0 ]; then
    echo "Run this with sudo." >&2
    exit 1
fi
rm -f /usr/share/wayland-sessions/osmia.desktop /usr/local/bin/osmia-session
rm -rf /usr/local/share/osmia
# Keep /usr/local/bin/osmia if you still run it from a shell; remove it with: rm /usr/local/bin/osmia
state=/var/lib/sddm/state.conf
if [ -f "$state.pre-osmia" ]; then
    mv -f "$state.pre-osmia" "$state"
    echo "Restored $state"
elif [ -f "$state" ] && grep -q 'osmia' "$state"; then
    sed -i '/^Session=.*osmia/d' "$state"
    echo "Removed the Osmia default from $state"
fi
echo "Done. Your own files in ~/.config/osmia are untouched."
