//! Clipboard history. A thread runs `wl-paste --watch`, which works with both
//! data-control protocols, and sends every text copy here. The history lives in
//! memory only, so nothing that was copied is written to disk.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use smithay_client_toolkit::reexports::calloop::channel::Sender;

/// Entries kept. The oldest is dropped past this.
pub const MAX_ENTRIES: usize = 50;
/// Larger copies are not kept, so a huge paste cannot fill memory.
pub const MAX_BYTES: usize = 256 * 1024;

pub struct Entry {
    pub text: String,
    pub at: Instant,
}

#[derive(Default)]
pub struct History {
    /// Newest first.
    pub entries: VecDeque<Entry>,
}

impl History {
    /// Add a copy. The same text again moves to the front instead of repeating.
    /// Returns false when the text was not kept.
    pub fn add(&mut self, text: String) -> bool {
        if text.trim().is_empty() || text.len() > MAX_BYTES {
            return false;
        }
        self.entries.retain(|e| e.text != text);
        self.entries.push_front(Entry { text, at: Instant::now() });
        self.entries.truncate(MAX_ENTRIES);
        true
    }

    pub fn remove(&mut self, i: usize) {
        self.entries.remove(i);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// One line for a list row: whitespace runs become a single space.
pub fn preview(text: &str) -> String {
    text.split_whitespace().take(60).collect::<Vec<_>>().join(" ")
}

/// How long ago, in a few characters.
pub fn age(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0..=4 => "now".into(),
        5..=59 => format!("{s}s"),
        60..=3599 => format!("{}m", s / 60),
        3600..=86399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

/// Runs for every change of the clipboard, with the content on stdin.
/// Copies a password manager marks as secret are skipped.
/// Each copy goes out followed by a NUL byte, which text almost never holds.
const WATCH_SCRIPT: &str = r#"wl-paste --list-types 2>/dev/null | grep -qi 'passwordManagerHint' && exit 0; head -c 262145; printf '\0'"#;

/// Start watching. Copies arrive on `tx`.
pub fn watch(tx: Sender<String>) {
    std::thread::spawn(move || loop {
        let mut cmd = Command::new("wl-paste");
        cmd.args(["--type", "text", "--watch", "sh", "-c", WATCH_SCRIPT])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // SAFETY: only calls prctl between fork and exec, so the watcher dies with the shell.
        unsafe {
            cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("clipboard history is off, wl-paste is not available: {e}");
                return;
            }
        };
        if let Some(out) = child.stdout.take() {
            let mut rd = BufReader::new(out);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match rd.read_until(0, &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                if buf.last() == Some(&0) {
                    buf.pop();
                }
                if buf.len() > MAX_BYTES {
                    continue;
                }
                if tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
                    let _ = child.kill();
                    return;
                }
            }
        }
        let _ = child.wait();
        // The compositor went away or wl-paste stopped. Try again shortly.
        std::thread::sleep(Duration::from_secs(2));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_text_moves_to_front() {
        let mut h = History::default();
        h.add("a".into());
        h.add("b".into());
        h.add("a".into());
        let v: Vec<_> = h.entries.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(v, ["a", "b"]);
    }

    #[test]
    fn blank_and_huge_are_skipped() {
        let mut h = History::default();
        assert!(!h.add("  \n".into()));
        assert!(!h.add("x".repeat(MAX_BYTES + 1)));
        assert!(h.entries.is_empty());
    }

    #[test]
    fn oldest_is_dropped() {
        let mut h = History::default();
        for i in 0..MAX_ENTRIES + 5 {
            h.add(i.to_string());
        }
        assert_eq!(h.entries.len(), MAX_ENTRIES);
        assert_eq!(h.entries.back().unwrap().text, "5");
    }

    #[test]
    fn preview_is_one_line() {
        assert_eq!(preview("a\n  b\t c"), "a b c");
    }
}
