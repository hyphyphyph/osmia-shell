# Overview

## Purpose

Show every open window at once, as a card with a picture of it, so the user can see them all and jump to one.

## Opening

Resting the pointer in the top left corner of the screen opens it. This is the hot corner. The pointer must stay in the corner, a square 4 px on a side, for 150 ms. A quick sweep to the bar does not open it. After it opens, it does not open again until the pointer has left the corner and moved on. The hot corner is part of the bar, so it works when the bar is on the top edge. With the bar at the bottom there is no hot corner, and a key binding is the way in. The command osmia overview toggle, show or hide does the same from a key binding or a script. The setting is overview.corner in the config, either top-left or off, and there is a Hot corner row for it in settings. The overview opens on the monitor that has focus, and shows the windows of every monitor.

## Look

The screen is dimmed with a dark layer over everything, and the cards sit on it. Each card is a picture of the window, with a line below it that holds the app icon and the window title. The title is cut by its width on screen, with an ellipsis. The picture keeps the shape of the window and is at most 16:10. It sits on a plate, with a thin outline. The window that has focus has a soft accent outline. The marked card has a strong accent outline. A minimized window shows its picture dimmed, with the word Minimized in the corner. At the top is a line that shows what the user has typed, or Type to filter windows, and a count of the windows shown. With no windows, it says No open windows. With a filter that matches nothing, it says No window matches.

## Layout

The cards are in a grid that is centred on the screen, with the most columns and rows that make the cards as big as they can be. A card is at most 480 px wide, so a single window does not fill the screen. The cards never overflow the screen. With very many windows they get small. The last row is centred.

## Pictures

The pictures come from the compositor through ext-image-copy-capture, one per window, taken once when the overview opens. This works for a window that is covered or minimized. Taking them happens on a thread, so the overview opens at once. Cards show the app icon, large, until their picture arrives, and each picture appears as it is ready. A window that cannot be captured keeps the app icon. A picture is matched to a window by its title and app id. Windows with the same title and app id take the pictures in order. The pictures are not live and are not updated while the overview is open. They are dropped when it closes, so nothing is kept in memory afterward. A picture is at most 560 px on its long side.

## Using

Clicking a card brings that window forward and closes the overview. A minimized window is restored first. Clicking the empty screen closes the overview. Middle-clicking a card, or clicking the x that shows on the hovered card, asks that window to close. It does not force it. Typing filters the cards. A window stays when every typed word is in its title or its app name, in any case. Backspace removes a letter. The arrow keys move the mark, and Enter brings the marked window forward. The pointer moves the mark when it moves. When it is still, the keys own the mark. Escape clears the filter first, and closes the overview when there is no filter. The list stays current. A window that opens or closes while the overview is showing is added or removed.

## Cost

The overview adds no timer and no polling. When it is closed, the only work is the hot corner check, which runs on pointer events in the corner. The pictures need one capture session per window while the overview opens, and nothing after.

## Tests

Unit tests cover the grid sizes, including that the cards never overflow, the filter, and the matching of pictures to windows, including windows that look the same. One test is run by hand with cargo test -- --ignored. It needs a running compositor, takes a picture of every open window, and checks that none is blank.

## Out Of Scope

- Windows moving or zooming in place, as the real windows. The overview is a layer of copies, so it is a card view and not an animation of the desktop.
- Showing where windows are on the desktop, or grouping them by workspace. The protocol does not say which workspace a window is on.
- Live pictures that update while the overview is open.
- A hot corner when the bar is at the bottom, or in other corners.

## Open Questions

- Whether to group the cards by workspace, from the focus history the shell already keeps.
- Whether a bottom bar should get a small surface of its own, so the hot corner works there.
- A key binding for it in the session. None is set yet.
- Whether Super+Tab, or another key, is the right one.
