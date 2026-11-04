# Bar

## Purpose

Show the windows that need the user, and the state of the machine, at a glance.

## Items

- Window attention list
- Clock and date
- System tray
- CPU monitor
- RAM monitor
- Battery
- Network
- Volume
- Brightness
- Notification history control
- Power button

## Window Attention List

Lists only two kinds of windows, minimized windows and windows in alert mode. It does not list every open window. A minimized window shows only its app icon. A window in alert mode opens up to show its app icon and a short title. Long titles are cut to fit. An entry in alert mode has a highlight color. Hover on an icon-only entry shows its title. Clicking an entry restores that window and gives it focus. Minimized state comes from the wlroots foreign-toplevel protocol. A window is in alert mode when it has a recent notification. The shell matches the notification app to the window. Alert mode ends when the window gets focus or the notification is dismissed. Alert mode does not use compositor-specific urgent flags. The match uses the desktop-entry hint in the notification first. If that is missing or finds nothing, it falls back to the notification app name. If neither matches a window, no window goes into alert mode.

## Clock And Date

Shows the current time and date. Clicking it opens a small calendar popup.

## System Tray

Shows tray icons from apps that use the StatusNotifierItem protocol on D-Bus. Clicking an icon activates the app. The shell reads the app menu over D-Bus and draws it with egui in a popup under the icon.

## Cpu And Ram Monitors

Show how much CPU and memory are in use. They read from the kernel and need no outside service. Each shows a short number, such as 12%. They update every 2 seconds. While their card is open, the card refreshes every second. Clicking one opens the Processor or Memory card, described in Processes.

## Status

Battery level comes from UPower. Network state comes from NetworkManager. Volume comes from PipeWire. Brightness comes from the kernel backlight. A missing source hides its item. It is never an error on screen.

## Input

Scrolling on volume changes the volume. Clicking it mutes. Scrolling on brightness changes the brightness. Clicking network opens a list of connections. The list shows saved networks and nearby networks. One click connects to a saved network. Joining a new network that needs a password is left to an outside tool.

## Power Button

Opens a menu with five actions, lock, log out, suspend, reboot, and power off. Lock and suspend run at once. Log out, reboot, and power off ask the user to confirm first. A lock action runs an outside locker. The shell does not draw a lock screen.

## Notification Entry

The bar shows a control to open the notification history. It also shows when do-not-disturb is on. It also shows a warning when another daemon blocked notifications.

## Placement

The bar sits on the top or bottom edge, as set in the config. It reserves its space so that windows do not cover it.

## Layout

The config sets the bar layout. The bar items are placed in left, center, and right lists. An item that is not listed is not shown. By default the window attention list is on the left. All other items are on the right, and the center is empty. The default order on the right, left to right, is tray, CPU, RAM, battery, network, volume, brightness, notifications, clock, power.

## Look

The bar is slim and dense, about 28 to 32 px tall. The bar is a flat strip. Items sit directly on it with no chip behind them. Thin dividers separate the groups, which are monitors, status items, and clock and power. The colors are dark neutral grey with one accent color. The accent marks alert entries and the do-not-disturb state. Status items show a small line icon and a short number, such as the battery icon and 82%. CPU and RAM show a short label and number, such as CPU 12%. The bar itself has square corners and a 1 px line on the edge that faces the screen. Attention entries and hover states have lightly rounded corners of 4 px. The right side is tight. CPU and RAM sit together in one group with a small gap. Battery, network, volume, and brightness sit together in one tight group. The tray, the monitors, the status group, and the clock and power group are set apart by thin dividers. When do-not-disturb is on, the notification control shows a crossed bell in the accent color. When it is off, the control shows a bell with a small accent badge for the unread count. A hovered item gets a plate with 4 px corners.

## Panels

The tray menu, the calendar, the network list, the power menu, and the Processor and Memory lists are floating cards. A card floats 6 px below the bar. It does not touch the bar. A card has 10 px rounded corners, a soft shadow, and a grey a little lighter than the bar. A card is about 250 px wide, with 6 px of padding. The Processor and Memory lists, and long tray menus, are wider when their content needs it. Rows are 32 px tall and have 6 px rounded corners. A hovered row gets a plate. Cards with a title show it in bold above the rows. The tray menu shows a check mark in front of the chosen item. The tray menu shows a chevron at the end of an item that opens a submenu. A disabled item is dimmed. A line separates groups of items. The calendar shows the month name with previous and next buttons, and a Monday-first grid. Today is filled with the accent color. The network list has a Saved group and a Nearby group. Each network shows its signal strength, and a lock mark when it needs a password. The connected network has a check mark. The network list ends with a note that new password networks need an outside tool. When the user picks log out, reboot, or power off, a confirm strip replaces the row. The strip is in the accent color and holds a Cancel button and a confirm button. Only one card is open at a time. A card that holds a list grows to fit it, up to the space on screen, and then scrolls.

## Mockups

The mockups are in spec/Bar.mockups/index.html. The picked variation of round 1 is spec/Bar.mockups/round-1/4.html. Round 1, variation 4 settled the right-hand layout, the icon-first attention list, and the do-not-disturb state. The picked variation of round 2, the latest, is spec/Bar.mockups/round-2/2.html. Round 2, variation 2 settled that the bar panels are floating cards.

## Out Of Scope

Workspace display is not in the first version.

## Open Questions

- Confirm the item behaviors and alert rules in ProposedDefaults.
- The exact accent color, the grey steps, and the font.
- Whether the notification popups and the launcher use the same floating card look.
