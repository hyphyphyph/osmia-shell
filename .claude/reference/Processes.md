# Processes

## Purpose

Show which programs use the processor and memory, and let the user stop a program or cap what it uses.

## Opening

Clicking the CPU or RAM item in the bar opens a floating card under it. The card is titled Processor or Memory. The header shows the machine total. Processor shows the share of all cores, such as 35% of 8 cores. Memory shows the used amount of the total.

## List

Processes are grouped by program name, and a program that runs as many processes shows as one row with a count. Every program is listed, ranked by the current measure, and the list scrolls. On Processor, a program using almost nothing is left out. Each row has a bar that is long in proportion to the busiest row. A row with many processes opens to show its processes one by one, with their command lines. Lock order holds the order still, so rows do not jump while the user aims at one. It lasts until the card closes. The list refreshes every second while the card is open.

## Units

Processor values are the share of the whole processor, where 100% means every core is busy. A program with several busy threads can use more than one core, and its share adds up across them, but never passes 100%. A value under 10% shows one decimal, such as 2.4%. Memory values are the resident memory of the program, without shared pages.

## Stopping

The x on a row asks for confirmation, then sends the program's processes a normal terminate signal. Stopping does not force-kill.

## Limits

A gauge button on a program row sets a limit for that measure. It shows on hover, and stays visible in the accent color when a limit exists. The editor replaces the row. It has a minus button, a field that takes a typed number, a plus button, a unit, and Set and Clear buttons. Clear reads Cancel when no limit exists yet. The field gets keyboard focus. Enter sets the limit. A value that is not usable outlines the field in the accent color and Set does nothing. The plus and minus buttons step through round values. Any value can also be typed. A processor limit is a share of the whole processor, from 0.1 to 100 percent. On 24 cores, 2% is about half a core. A memory limit is in megabytes, 16 or more. The row's memory figure goes from 412 MB to 412 MB / 512 MB when a limit exists. A program can have a processor limit and a memory limit at the same time. A limit covers every process of the program, including processes that start later. The shell checks for new ones every few seconds even when the card is closed. Programs with a limit are listed first, and a limited program that is not running still has a row marked not running, so its limit can be cleared. Clearing a limit sends the processes back to the control group they came from.

## How Limits Work

The shell uses cgroup v2 in the user's own part of the tree, so it needs no root rights. It creates one group per limited program under the user's systemd service, named osmia-limits, and moves the program's processes into it. The processor limit is the cpu.max setting. The program is held to its quota and is not stopped or killed. The memory limit is the memory.high setting. Past it, the kernel reclaims memory and slows the program. It does not kill the program. A group with no limit and no processes is removed. If the session has no delegated cpu and memory controllers, setting a limit fails with a notification that says why. Everything else in the card works. If some processes cannot be moved, the notification says how many, and the rest stay limited.

## Persistence

Limits are kept in a small file named proc-limits under the user state folder, in the osmia folder. They are applied again when the shell starts and when a limited program starts. A limit follows the program name, so it applies to the next run of that program.

## Sizing

Cards that hold a list take as much height as the list needs, up to the space between the bar and the screen edge, and then scroll. This holds for the notification history, the window list, the tray menu, and this card.

## Tests

The path that parses and converts limits is covered by unit tests. One test needs a systemd session and is run by hand with cargo test -- --ignored. It caps a busy loop and checks that it uses the capped share, then checks that clearing puts it back.

## Out Of Scope

- A limit that kills the program when it passes memory. memory.max would do this.
- Limits on disk or network use.
- Limiting a program that runs in another user session.

## Open Questions

- Whether a hard memory limit, which can kill the program, should be an option beside the soft one.
- Whether limits should reach programs by their service or application name rather than the process name, for programs that rename their processes.
- If a limited process ends up in a systemd scope that the shell moved it out of, whether systemd tools should be told.
