# Proposed Defaults

## Status

These are proposals, not confirmed decisions. They fill gaps left by the interview. They do not override any confirmed decision. Confirmed decisions are in Shell, Bar, Launcher, Notifications, and Control.

## Control

- Commands are launcher toggle, show, hide. history toggle, show, hide, clear. settings with an optional page. dnd on, off, toggle, status. popups clear. config reload, check. status.
- Running osmia with no arguments starts the shell. Running it with a part and action acts as the client.
- Starting a second shell prints a message and exits, because one is already running.
- Exit codes are 0 for done, 1 for a bad command or failure, and 2 when no shell is running.
- A json flag makes status print JSON. It holds the do-not-disturb state, popup count, history count, and the monitor names.
- A control command can clear the popups on screen.

## Config

- The config has these tables, bar, theme, notifications, launcher, session, and alerts.
- An unknown key is ignored and a warning notification names it. The rest of the file still applies.
- The lock command is loginctl lock-session by default, and the user can change it.
- The log out command ends the current session through loginctl by default, and the user can change it.
- Suspend, reboot, and power off go through logind on D-Bus, so no root rights are needed.
- The popup corner can be set to any of the four corners. The default is bottom right.
- The alerts table is optional and empty by default. It maps a notification app name to a window app id.
- The default theme is dark. A built-in light theme can be picked by name. There is no automatic switch.
- The font is the system sans-serif font from fontconfig, with a small bundled font as fallback.
- The default bar height is 32 logical pixels.
- Apps that need a terminal run in the terminal named in the launcher table, which defaults to the TERMINAL variable.

## Keys

- The session binds Super+Comma to osmia settings. Super alone still opens the launcher.

## Bar

- The bar sits on the top layer. Popups, menus, and the launcher sit on the overlay layer.
- Only one bar popup or menu is open at a time. It closes on Escape, on a click outside, or when another opens.
- Battery shows the percent and whether it is charging. Below 15 percent it uses the highlight color. With no battery it is hidden.
- Network shows wired or wifi, the signal strength for wifi, and an offline icon when not connected.
- Volume shows the percent and a muted icon. It controls the default output only, with no device picker. One scroll step changes it by 5 percent.
- Brightness shows only when a backlight exists. It is changed through logind on D-Bus, so no root rights are needed.
- CPU shows the total use across all cores. RAM shows the share of memory that is not available.
- The clock shows 24-hour time as HH:MM by default, and the user can change the format.
- The calendar popup shows the current month with previous and next buttons. It has no events.
- If no StatusNotifierWatcher exists on D-Bus, the shell provides one.
- Tray icons fit the bar height, use named or pixmap icons, and show a tooltip on hover.
- Tray menus support submenus, separators, check items, and disabled items.
- The attention list orders entries by when they entered the list. It shows at most 8 entries, then a count such as +3. Titles are cut at 24 characters.
- The notification control shows a count of unread notifications. The count resets when the history panel is opened.

## Launcher

- Ranking boosts the match score by how often the user opens an item. The counts are kept in a small file under the user state folder, and entries unused for 90 days fade out.
- The prefixes are the greater-than sign for commands, the at sign for windows, and the colon for quick actions. The calculator needs no prefix.
- With an empty search box, the launcher shows the 8 most used apps.
- The launcher shows at most 8 results and scrolls for more.
- The calculator handles plus, minus, times, divide, remainder, power, and brackets. It has no units. Enter copies the answer to the clipboard and closes the launcher.
- Desktop entries marked Hidden, NoDisplay, or not for this desktop are not listed. Exec field codes are removed before running.
- Programs the launcher starts run in their own session and keep running if the shell exits.
- Picking an open window activates it and restores it if it is minimized.
- The launcher closes after running something, on Escape, and when it loses focus.
- The launcher is 600 logical pixels wide, centered across, in the upper third of the screen.
- Icons come from the user's icon theme. They load only when shown and are cached by size.

## Notifications

- A notification with no timeout lasts 5 seconds. A critical one never expires by itself.
- The shell advertises body, actions, and basic markup. Markup is limited to bold, italic, and underline. It shows the app icon only, with no images and no sound.
- A notification that replaces an earlier one by id updates it in place. A transient notification is not saved to history.
- A popup is 360 logical pixels wide. The body shows at most 4 lines and ends with an ellipsis if cut.
- The closed signal says expired, dismissed by the user, or closed by the app.
- The history panel opens next to the bar item. It lists newest first, with app name, summary, body, and time. Clicking an entry brings its window forward.
- Do-not-disturb is not saved. It is off every time the shell starts.

## Shell

- Logs go to standard error, where journald collects them. The level comes from the RUST_LOG variable. There is no log file.
- The shell needs layer-shell and foreign-toplevel from the compositor. It uses fractional scale and viewporter when offered. It starts without foreign-toplevel but hides the attention list and open-window search, and logs a warning.
- Screen reader support is out of scope for the first version.
