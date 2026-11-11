# Control

## Purpose

Let a compositor key binding, or a script, tell the running shell what to do. Wayland apps cannot grab global keys, so this is how the launcher is opened.

## How

The shell binary also works as a command line client. The client sends the request over a Unix socket to the running shell. The binary is named osmia. A command is written as osmia, then the part, then the action. Examples are osmia launcher toggle, osmia dnd on, and osmia history toggle. osmia overview and osmia launcher are the same screen, the window overview with the launcher search. Their actions are toggle, show, and hide. osmia settings opens the settings window, or brings it forward if it is already open. An optional page name opens that page.

## Socket

The socket is in the user runtime folder, for example XDG_RUNTIME_DIR. Only the owner of the session can use it. There is no way to open it to other users.

## Status

The client can also ask the shell for its state. An example is osmia dnd status, which says whether do-not-disturb is on. The answer is plain text, or JSON when the user asks for it. There is no live event stream in the first version.

## Commands

- Toggle the launcher
- Toggle the notification history
- Turn do-not-disturb on, off, or toggle it
- Reload the config
- Ask for the state of a part
- Open the graphics card, or ask its state, as described in GraphicsCard
- Open the Bluetooth card, ask its state, or turn it on or off, as described in Bluetooth
- Step the volume, brightness, or keyboard light, mute, or ask their state, as described in Osd
- Sign in to Spotify, and like or unlike the playing song, as described in Spotify

## Failure

If no shell is running, the client prints an error and exits with a failure code.

## Open Questions

- Confirm the commands, exit codes, and JSON status in ProposedDefaults.
- Short forms for commands.
- Whether a live event stream is added later.
