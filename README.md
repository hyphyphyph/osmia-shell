# osmia

A very light shell for wlroots-style Wayland compositors (labwc, Sway, Hyprland, niri, river).
Rust, egui, one process. It gives you a bar, a launcher, and a notification daemon.

The specification is in `spec/` (Piton). Mockups are in `spec/*.mockups/`.

## Build and run

    cargo build --release
    ./target/release/osmia          # starts the shell
    cargo install --path .          # optional: puts `osmia` in ~/.cargo/bin

Start it from your compositor's autostart, or use `contrib/osmia.service`.
Stop any other notification daemon first. If one is running, osmia keeps working
without notifications and the bar shows a "Blocked" item.

## Login session (SDDM)

An "Osmia" entry for the login screen: labwc plus the shell, with its own compositor config in
`~/.config/osmia/labwc/` (copied on first login, yours after that). Your LXQt session is not touched.

    cargo build --release
    sudo sh contrib/session/install.sh      # installs the entry and makes Osmia the SDDM default
    sudo sh contrib/session/uninstall.sh    # removes it and restores the old default

It installs `/usr/local/bin/osmia`, `/usr/local/bin/osmia-session`,
`/usr/share/wayland-sessions/osmia.desktop`, and default config in `/usr/local/share/osmia/labwc`.
Super opens the launcher. Super+Comma opens Osmia settings. Click the brightness item in the bar to set the screen timeout (osmia runs `swayidle` for this and keeps the choice in `~/.local/state/osmia/screen-timeout`). Run the install again after you rebuild, to update the binary.

## Screenshots and video

Built in. Needs a compositor with `ext-image-copy-capture` (labwc 0.9 and newer, and others), `ffmpeg` for video
and `wl-copy` for the clipboard.

| Key (session default) | What it does |
| --- | --- |
| Print | Opens the capture overlay: Area, Window or Screen, Photo or Video, sound, cursor and a delay |
| Shift+Print | The overlay on Area with Video chosen |
| Alt+Print | Screenshot of the focused window, no overlay |
| Ctrl+Print | Screenshot of the whole screen, no overlay |

Any of these while recording stops it, and so does the red timer in the bar. Screenshots go to `~/Pictures/Screenshots`
(and the clipboard), recordings to `~/Videos` as MP4 with hardware encoding when it works. The notification has
buttons to open the file or folder and to convert a recording to GIF or WebM. Everything is under
"Screenshots and video" in the settings window, or `[capture]` in the config file.

`osmia grab [--list] [--output NAME | --window ID] [--cursor] [--video SECONDS [--audio system|mic|both]] FILE`
takes a picture or a short recording without the shell, which is handy in scripts.

## Window menu

The grid button at the left end of the bar lists every open window, grouped by
workspace and then by application. Click a window to focus it, restoring it if
it was minimized. Click a workspace name to switch to it. With one workspace
there are no workspace headings. `osmia windows` toggles the menu.
labwc does not say which workspace a window is on, so osmia notes the workspace
that was active when a window opened or was last focused. Windows that were
already open when osmia started are listed under "Other workspaces" until you
focus them.

## Keyboard shortcuts

Settings has a Keyboard shortcuts page. It reads the `<keyboard>` section of
labwc's `rc.xml` (`~/.config/osmia/labwc/rc.xml`) and edits it in place, so
your comments and other settings stay. You can change the keys, change the
command a shortcut runs, remove one, add one, or restore the shipped list.
Click Record and press the keys, or type a name such as `W-S-s` or
`XF86_AudioPlay`. labwc reloads at once. Shortcuts that are already bound run
instead of being recorded, so type those. Two shortcuts with the same keys are
flagged. The first one wins. The first save keeps a copy as `rc.xml.bak-osmia`.

`osmia settings [page]` opens the settings window, for example `osmia settings shortcuts`.

## Media players

Anything that follows MPRIS (Spotify, browsers, mpv and most others) shows up three ways:
a "Now playing" item in the bar (click for cover art, a seek bar and the buttons; middle-click plays or pauses;
right-click skips), the same controls inside the tray panel of the app that owns the player, and the media keys.
`osmia media play-pause|pause|next|previous|status` does the same from a terminal or a key binding.

## Control (bind these to keys in your compositor)

    osmia media [play-pause|pause|next|previous|status|panel]
    osmia capture [area|window|screen|video|region-video|window-video|screen-video|now screen|now window|stop]
    osmia capture pick screen|area X Y W H|window N     # choose in the open overlay, for scripts
    osmia launcher toggle|show|hide
    osmia history toggle|show|hide|clear
    osmia clipboard toggle|show|hide     # clipboard history (Super+V in the session)
    osmia emoji toggle|show|hide         # emoji picker (Super+. in the session)
    osmia overview toggle|show|hide
    osmia settings [page]                # opens settings (Super+Comma in the session)
    osmia dnd on|off|toggle|status
    osmia popups clear
    osmia config reload|check
    osmia status [--json]

Exit codes: 0 done, 1 bad command or failure, 2 no shell running.
The socket is `$XDG_RUNTIME_DIR/osmia.sock`, owner only.

## Config

`~/.config/osmia/config.toml`. A missing file means defaults. The shell reloads it when the file
changes. A bad edit keeps the last good config and shows a notification. Example:

    [bar]
    edge = "top"                 # or "bottom"
    height = 30
    clock_format = "%H:%M"

    [bar.layout]
    left   = ["attention"]
    center = []
    right  = ["tray", "|", "cpu", "ram", "|", "battery", "network", "volume", "brightness",
              "notifications", "|", "clock", "power"]

    [theme]
    accent = "#f0a35e"

    [notifications]
    corner = "bottom-right"      # top-left, top-right, bottom-left, bottom-right
    default_timeout_ms = 5000
    history_size = 100

    [launcher]
    terminal = "alacritty"

    [session]
    lock = "loginctl lock-session"
    logout = "loginctl terminate-session self"

    [alerts]                     # notification app name -> window app id
    "Some App" = "some-app"

## Needs

A compositor with `wlr-layer-shell`. Optional: `wlr-foreign-toplevel-management` (window list and
alerts), `wpctl` and `pactl` (volume), `nmcli` (network), `wl-copy` (calculator copy).

## Tests

    cargo test

Visual tests run in a headless `labwc` on a private D-Bus. See `spec/` for what each part must do.

## Overview

The overview and the launcher are one screen, as in GNOME. Press Super, or rest the pointer in the top left
corner (the bar must be on top), to see every open window as a card, with the search box above. Click a card to switch to it, or middle-click it (or use the x) to close that window. Start
typing and the cards give way to search results for apps, windows, actions and sums. The first Escape clears
the text, the next leaves. Turn the corner off under Launcher in settings (`overview.corner = "off"`).
`osmia overview toggle` and `osmia launcher toggle` do the same thing. See `spec/Overview.pi`.

## Default applications

Settings has a Default applications page for the web browser, email, file manager, text editor,
documents, images, video and music, plus the terminal. It reads the standard `mimeapps.list` files and
sets a choice with `xdg-mime default`, so every program sees it. `osmia settings default` opens it.
See `spec/DefaultApps.pi`.

## Process limits

Click CPU or RAM in the bar to see which programs use the most. The gauge button on a row sets a limit
for that program: CPU is a share of the whole processor, memory is in MB. Limits use cgroup v2 in your
user session (no root) and are kept in `$XDG_RUNTIME_DIR/osmia/proc-limits` (so they are lost at reboot, like the cgroups). Memory is a soft limit
(`memory.high`): the program slows down, it is not killed. See `spec/Processes.pi`.

    cargo test -- --ignored      # also runs the CPU limit test (needs a systemd session)

## Bluetooth

The bar has a Bluetooth item. Its card turns Bluetooth on and off, connects and
forgets paired devices, and pairs new ones; it looks for devices while it is open.
osmia is also the pairing agent, so codes to confirm or type show up in that card,
and blueman-applet is not needed. Do not run another agent beside it, or it may
take the prompts. `osmia bluetooth`, `osmia bluetooth status` and
`osmia bluetooth on|off` drive it from a script. `OSMIA_NO_BLUETOOTH=1` turns it off.

## Graphics card for VMs

On a machine with the `gpu-vfio-control` service, the bar shows a **GPU** item
(Host, VFIO, or the VM's name). Its card hands the NVIDIA card to VMs and takes
it back, starts and stops the VM, and can wait to hand the card over until
nothing holds it. The service does the binding and the safety checks; osmia only
talks to it over D-Bus, and polkit decides who may.

The card can only move while the desktop is off it, so `osmia-session` keeps
labwc, EGL and Vulkan away from it when `/etc/gpu-vfio-control.toml` exists
(set `OSMIA_KEEP_DGPU=1` to opt out). The HDMI port is wired to that card, so it
does not work on the desktop. Run one program on the card with `osmia-dgpu <program>`.

Set up the machine once with `sudo sh contrib/gpu/setup.sh`. It unblacklists
nvidia, stops vfio-pci claiming the card at boot, keeps the login screen's X
server off the card, enables the NVIDIA suspend services, and rebuilds the
initramfs. `osmia gpu` opens the card; `osmia gpu status` prints the state.
