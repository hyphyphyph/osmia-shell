# Notifications

## Purpose

Show messages from apps, and keep the ones the user missed.

## Daemon

The shell owns the org.freedesktop.Notifications name on the session D-Bus. Any app that uses the standard notification API reaches it.

## Popups

Each notification shows as a popup on the monitor that has focus. Popups sit at the bottom right of the screen and stack. On a monitor with a bottom bar, they sit above the space the bar reserves. A popup honors the timeout the app asked for. A popup honors urgency. Critical ones do not expire by themselves. A popup shows the action buttons the app supplied. When the user picks an action, the shell tells the app. A click on the popup runs the default action the app gave, if any. The click also brings the matching window forward, then dismisses the popup. A right-click dismisses the popup and does nothing else. The popups show as many as fit on the screen. After that, new ones overlap the older ones in a pile, with the newest on top. In the pile, the newest popup shows as a full card. The older popups show as readable strips above it. Each strip is 20 px tall and shows the app icon and the summary on one line. The summary is cut with an ellipsis if it is too long. At most 3 strips show. Each strip is a little narrower than the one below it. If more than 3 older popups exist, a line above the strips says how many more, such as +1 more. The clear-all button sits on that same line, on the right. Hovering over the pile spreads it out, so each popup can be read and clicked. In the spread view, the newest popup is a full card with its action buttons. The older popups are one-line rows that show the app icon, app name, summary, and age. A row is 30 px tall. Clicking a row runs the same click action as a card. In the spread view, the clear-all button sits at the top right of the pile. The top of a long spread pile fades out to show that it scrolls. A card has 10 px rounded corners, a shadow, and the same floating card look as the bar panels. A critical popup has a 3 px accent edge on its left side and an accent app name. The pile collapses again when the pointer leaves it. If the spread pile is taller than the screen, the user scrolls it with the wheel. The newest popup stays nearest the bottom edge. Each popup keeps its own timer, even when it is covered. Hovering over the pile pauses every timer. A popup that expires leaves the pile and stays in the history. Critical popups never expire by themselves. When the pile holds two or more popups, it shows a clear-all button. Clear-all removes the popups from the screen and keeps them in the history.

## History

A panel opened from the bar lists past notifications. The user can dismiss one or clear the list. History is kept in memory only, and holds at most 100 entries. When it is full, the oldest entry goes first. History is lost when the shell restarts.

## History Panel

The panel is a floating card under the bell, at the right end of the bar. It is 420 px wide and shows at most about 390 px of entries. The rest scrolls. The bottom of the list fades out to show that it scrolls. The header shows the title Notifications and a count of how many are kept out of 100. Entries are listed newest first. Each entry shows the app icon, app name, age, summary, and body. An unread entry has a small accent dot. A critical entry has a 3 px accent edge on its left. An entry that has actions keeps its action buttons. A small close mark shows on an entry when the pointer is over it. It removes that entry. The footer holds the do-not-disturb switch on the left and the Clear all button on the right. When the history is empty, the panel says Nothing missed and explains that missed items wait there. When do-not-disturb is on and the history is empty, the empty text also says that popups are hidden and critical ones still show. The switch is in the accent color when do-not-disturb is on.

## Name Conflict

If another notification daemon already owns the name, the shell does not take it. The shell keeps running without notifications and logs a warning. The bar shows a warning item so the user can see why.

## Window Alerts

A notification can put its app window into alert mode in the bar. The shell finds the window from the desktop-entry hint, then from the app name.

## Do Not Disturb

The user can turn do-not-disturb on or off. While on, popups are hidden and notifications still go to the history. Critical notifications still show as popups while it is on. The bar shows when it is on.

## Idle Cost

With no popup showing, the notification part draws nothing.

## Mockups

The mockups are in spec/Notifications.mockups/index.html. The picked variation of round 1 is spec/Notifications.mockups/round-1/3.html. Round 1, variation 3 settled the readable strips in the pile and the one-line rows in the spread view. The picked variation of round 2, the latest, is spec/Notifications.mockups/round-2/3.html. Round 2, variation 3 settled the wide history panel with actions and a footer.

## Out Of Scope

- Grouping by app
- Inline reply
- Rich images

## Open Questions

- Confirm the popup corner, history panel, and timeout choices in ProposedDefaults.
