# Clipboard

## Purpose

Let the user get back something they copied earlier, without going back to where it came from.

## Opening

Super+V opens it. The session binds Super+V to osmia clipboard toggle. The commands osmia clipboard toggle, show and hide work from any key. It opens on the monitor that has focus, and the same key closes it.

## Watching

The shell watches the clipboard for as long as it runs, using wl-paste --watch, which works with both data-control protocols. Only text is kept. Pictures and files on the clipboard are not. A copy that a password manager marks as secret is not kept. A copy that is blank, or larger than 256 KB, is not kept. Copying the same text again moves it to the top instead of adding a second line. The newest 50 copies are kept. The oldest is dropped past that.

## Privacy

The history is kept in memory only. It is never written to disk, and it is gone when the shell stops.

## Look

A card at the top of the screen, 36 px down, 480 px wide, over the same dimmed screen as the overview. It has the look of the launcher card. A header reads Clipboard and has a Clear all button on the right. Each copy is one row, 30 px tall, newest first. Line breaks and runs of spaces show as one space, and a long copy is cut with an ellipsis. A row shows how long ago it was copied on the right. The hovered or marked row shows an x there instead. The marked row has a lighter plate and a 2 px accent edge on its left. The list scrolls when it is longer than ten rows or the screen. With nothing copied yet, the card says Nothing copied yet. A footer lists the keys.

## Using

Clicking a row, or Enter on the marked row, puts that text on the clipboard again and closes the card. The user then pastes with the usual key. The shell does not type into other windows. Up and Down, or Ctrl+N and Ctrl+P, move the mark. The pointer moves it when it moves. Delete, or a click on the x, removes the marked row. Clear all empties the list. Escape, a click on the empty screen, or Super+V again closes it. The list stays current. A copy made while the card is showing is added at once.

## Cost

No timer and no polling. One wl-paste process waits for the clipboard to change, and it exits with the shell.

## Tests

Unit tests cover moving a repeated copy to the top, skipping blank and oversize copies, dropping the oldest past 50, and the one-line preview.

## Out Of Scope

- Pictures and files in the history.
- Keeping the history across restarts.
- Pasting into the focused window for the user.
- Searching the history by typing.

## Open Questions

- Whether the history should survive a restart, and if so, how to keep secrets out of it.
- Whether typing should filter the list.
- Whether the size of the list should be a setting.
