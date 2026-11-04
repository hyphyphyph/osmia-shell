# Default Apps

## Purpose

Let the user choose which app opens a web link, a folder, a PDF and so on, without leaving the shell settings.

## Page

Settings has a page named Default applications, between Launcher and Screenshots and video. Each row has a name, one line of help, and a drop-down list on the right. The drop-down shows the current app by name, or Not set. Choosing an entry sets it at once, with no Apply button.

## Rows

- Web browser: web links and HTML files.
- Email: mailto links.
- File manager: folders.
- Text editor: plain text files.
- Documents: PDF files.
- Images: PNG, JPEG, GIF and WebP.
- Video: MP4, MKV and WebM.
- Music: MP3, FLAC, Ogg and WAV.
- Terminal: a text field. It is the terminal that apps needing one run in. It is the launcher terminal setting, and empty means the TERMINAL variable.

A choice covers every type listed for the row, so choosing a browser sets web links and HTML files together.

## Candidates

The list holds the installed apps whose desktop file says it opens the row's main type, sorted by name. Apps that are hidden, or are not applications, are left out. The current default always appears in the list, even when its desktop file does not name the type. A row with no candidates says that no installed app opens these.

## Reading

The current app follows the standard rules. The first installed app under Default Applications wins, then Added Associations, then the system's cache. Earlier files win over later files. The files are mimeapps.list in the user config folder, the system config folders, and the application folders, with a desktop-specific file first. An app named in these files that is not installed is skipped. The page reads the files each time settings opens.

## Writing

The shell sets a default by running xdg-mime default with the app and the row's types. That writes the user's mimeapps.list, so every other program sees the change. The page shows the new choice at once and does not wait for the command. The shell edits no mimeapps.list file itself.

## Tests

Unit tests cover desktop file reading, and the order that default, added and cache entries and earlier files win. One test is run by hand with cargo test -- --ignored. It checks every row against xdg-mime query default on the machine.

## Out Of Scope

- Rows for other file types, or a way to add one.
- Choosing an app for a single file. This is a default for a type.
- Removing an app from the list.

## Open Questions

- Whether to add a Browse all types view, for types without a row.
- Whether the shell should read the new default back after setting it, to catch a failed command.
