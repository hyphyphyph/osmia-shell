#!/bin/sh
# Install the Osmia login session and make it the default in SDDM.
# Run it with: sudo sh contrib/session/install.sh
# Undo it with: sudo sh contrib/session/uninstall.sh
set -eu

if [ "$(id -u)" -ne 0 ]; then
    echo "Run this with sudo." >&2
    exit 1
fi

here=$(cd "$(dirname "$0")" && pwd)
repo=$(dirname "$(dirname "$here")")
bin="$repo/target/release/osmia"
user="${SUDO_USER:-}"

if [ ! -x "$bin" ]; then
    echo "Build first: cargo build --release  (looked for $bin)" >&2
    exit 1
fi

echo "Installing files"
install -Dm755 "$bin" /usr/local/bin/osmia
install -Dm755 "$here/osmia-session" /usr/local/bin/osmia-session
install -Dm644 "$here/osmia.desktop" /usr/share/wayland-sessions/osmia.desktop
install -d /usr/local/share/osmia/labwc
install -m644 "$here"/labwc/* /usr/local/share/osmia/labwc/

# SDDM keeps the session you used last, and the login screen starts on it.
# Setting it here makes Osmia the default.
state=/var/lib/sddm/state.conf
entry=/usr/share/wayland-sessions/osmia.desktop
if [ -n "$user" ]; then
    mkdir -p /var/lib/sddm
    if [ -f "$state" ]; then
        [ -f "$state.pre-osmia" ] || cp -p "$state" "$state.pre-osmia"
        current=$(sed -n 's/^Session=//p' "$state" | head -1)
        case "$current" in
            /*) value="$entry" ;;                 # SDDM stores full paths
            "") value="$entry" ;;
            *)  value="osmia.desktop" ;;          # SDDM stores file names
        esac
        if grep -q '^Session=' "$state"; then
            sed -i "s|^Session=.*|Session=$value|" "$state"
        elif grep -q '^\[Last\]' "$state"; then
            sed -i "/^\[Last\]/a Session=$value" "$state"
        else
            printf '\n[Last]\nSession=%s\n' "$value" >> "$state"
        fi
        if grep -q '^User=' "$state"; then
            sed -i "s|^User=.*|User=$user|" "$state"
        else
            sed -i "/^\[Last\]/a User=$user" "$state"
        fi
    else
        printf '[Last]\nSession=%s\nUser=%s\n' "$entry" "$user" > "$state"
    fi
    echo "SDDM will start on Osmia for $user. State file:"
    sed 's/^/    /' "$state"
else
    echo "No user name known (SUDO_USER is empty). Osmia is installed but not the default."
fi

echo "Done. Log out to see the Osmia entry on the login screen."
