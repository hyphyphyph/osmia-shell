# Shell

## Purpose

A very light desktop shell for Wayland, written in Rust with egui. It gives the user a bar, a launcher, and notifications.

## Runs On

Any wlroots-style compositor, such as Sway, Hyprland, niri, and river. It uses the layer-shell protocol to place its surfaces. It does not need to run on GNOME or KDE.

## Process

The whole shell is one Rust process. That process owns every surface, and all of them share one state. There is one binary. The same binary is also the control client.

## Parts

- Bar
- Launcher
- Notifications
- Control client

## Lightness

The shell must be light in three ways, in this order of importance. It must cost almost nothing when idle. It must stay small in memory. It must open the launcher fast. The starting goals are under 100 MB resident and a launcher that shows in under 50 ms. These are goals to measure, not hard limits.

## Idle Behavior

The shell draws a frame only when something changed. While nothing changes, it does not wake the CPU or the GPU.

## Rendering

egui draws on the GPU by default. If no GPU is available, the shell falls back to software drawing.

## Success Criteria

- The bar shows on every monitor on a wlroots-style compositor.
- A notification from any app using the standard D-Bus API appears as a popup.
- A key bound in the compositor opens the launcher through the control client.
- With nothing changing on screen, the shell does no drawing and no polling. The only timers are the clock minute and the 2-second CPU and RAM readout.

## Non Goals

- Lock screen. The launcher quick action runs an outside locker.
- Wallpaper. Use an outside tool.
- On-screen display for volume and brightness changes.
- Clipboard history and screenshots.
- Plugins. Users cannot load code into the shell.
- Workspace display in the bar for the first version.
- Support for GNOME and KDE.

## Config

There is one config file in TOML, at ~/.config/osmia/config.toml. It sets the bar layout, the bar edge, and the theme. The shell reloads it when the file is saved. If the file is missing, the shell uses built-in defaults. If a change has an error, the shell keeps the last good config. It then shows a notification that says what was wrong. The theme can set colors, font, font size, bar height, and corner radius. The theme cannot set style rules for single items. Users cannot extend the shell with scripts or plugins.

## Scaling

The shell supports fractional scaling for each monitor, with sharp text. It uses the fractional-scale protocol where the compositor offers it. Where it does not, the shell uses the whole-number scale.

## Startup

The compositor starts the shell by running osmia. A user systemd unit is optional, and can restart the shell after a crash. The shell does not watch or restart itself.

## Monitors

Every monitor gets its own bar. When a monitor is plugged in, its bar appears at once. When a monitor is removed, its bar goes away at once. No restart is needed. If the last monitor goes away, the shell stays alive and waits for a new one. Open popups and the launcher move to the focused monitor. The bar edge, top or bottom, is set in the config. Notification popups and the launcher show on the monitor that has focus.

## Open Questions

- Confirm the choices in ProposedDefaults, which the user has not yet reviewed.
- The full list of config options and their default values.
- Which GPU stack egui uses on Wayland, and how the fallback is chosen.
- The look and theme of the shell. This is a job for the crux-ui-styles skill.
- Whether the bar can show workspaces later, and from which source.
- Whether the GPU context pushes memory past the 100 MB goal, since it adds cost.
