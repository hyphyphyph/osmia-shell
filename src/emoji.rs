//! The emoji picker's data: the built-in list, search, skin tones, and the recent list.
//! The list in data/emoji.tsv is made from Unicode emoji-test.txt and the English CLDR
//! annotations by contrib/tools/gen_emoji.py. Do not edit it by hand.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Picks kept in the recent list.
pub const MAX_RECENT: usize = 30;

const DATA: &str = include_str!("../data/emoji.tsv");

pub struct Emoji {
    pub text: &'static str,
    pub group: usize,
    pub name: &'static str,
    /// Lowercase words of the name, then of the keywords, for search.
    name_words: Vec<String>,
    key_words: Vec<String>,
    /// The five skin tones, light to dark, when it has them.
    pub tones: Option<[&'static str; 5]>,
}

impl Emoji {
    /// This emoji in a skin tone (0 to 4), or plain for None or when it has no tones.
    pub fn with_tone(&self, tone: Option<usize>) -> &'static str {
        match (tone, &self.tones) {
            (Some(t), Some(ts)) if t < 5 => ts[t],
            _ => self.text,
        }
    }
}

pub struct List {
    pub groups: Vec<&'static str>,
    pub emoji: Vec<Emoji>,
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || c == ':' || c == ',' || c == '“' || c == '”')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

fn parse(text: &'static str) -> List {
    let mut groups = Vec::new();
    let mut emoji = Vec::new();
    for line in text.lines() {
        if let Some(g) = line.strip_prefix("# groups\t") {
            groups = g.split('\t').collect();
            continue;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut it = line.split('\t');
        let (Some(text), Some(group), Some(name)) = (it.next(), it.next(), it.next()) else { continue };
        let keys = it.next().unwrap_or("");
        let tones: Vec<&'static str> = it.next().unwrap_or("").split(' ').filter(|t| !t.is_empty()).collect();
        emoji.push(Emoji {
            text,
            group: group.parse().unwrap_or(0),
            name,
            name_words: words(name),
            key_words: words(keys),
            tones: <[&str; 5]>::try_from(tones).ok(),
        });
    }
    List { groups, emoji }
}

/// The built-in list, read once.
pub fn list() -> &'static List {
    static LIST: OnceLock<List> = OnceLock::new();
    LIST.get_or_init(|| parse(DATA))
}

/// Indexes into the list that match the query, best first. Every typed word must start
/// a word of the name or a keyword. Name matches come before keyword matches.
pub fn search(list: &List, query: &str) -> Vec<usize> {
    let q = words(query);
    if q.is_empty() {
        return Vec::new();
    }
    let starts = |ws: &[String], w: &str| ws.iter().any(|x| x.starts_with(w));
    let mut by_name = Vec::new();
    let mut by_key = Vec::new();
    for (i, e) in list.emoji.iter().enumerate() {
        if q.iter().all(|w| starts(&e.name_words, w)) {
            by_name.push(i);
        } else if q.iter().all(|w| starts(&e.name_words, w) || starts(&e.key_words, w)) {
            by_key.push(i);
        }
    }
    by_name.extend(by_key);
    by_name
}

/// What the picker remembers: recent picks (as indexes into the list) and the skin tone.
#[derive(Default)]
pub struct Memory {
    /// Newest first.
    pub recent: VecDeque<usize>,
    pub tone: Option<usize>,
}

fn state_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("osmia").join("emoji"))
}

impl Memory {
    pub fn load() -> Self {
        let text = state_file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
        Self::from_text(list(), &text)
    }

    fn from_text(list: &List, text: &str) -> Self {
        let mut m = Memory::default();
        for line in text.lines() {
            if let Some(t) = line.strip_prefix("tone=") {
                m.tone = t.trim().parse().ok().filter(|t| *t < 5);
            } else if let Some(i) = list.emoji.iter().position(|e| e.text == line.trim())
                && !m.recent.contains(&i)
                && m.recent.len() < MAX_RECENT
            {
                m.recent.push_back(i);
            }
        }
        m
    }

    fn to_text(&self, list: &List) -> String {
        let mut out = String::new();
        if let Some(t) = self.tone {
            out.push_str(&format!("tone={t}\n"));
        }
        for &i in &self.recent {
            if let Some(e) = list.emoji.get(i) {
                out.push_str(e.text);
                out.push('\n');
            }
        }
        out
    }

    pub fn save(&self) {
        let Some(f) = state_file() else { return };
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, self.to_text(list()));
    }

    /// Note a pick. A repeat moves to the front.
    pub fn picked(&mut self, i: usize) {
        self.recent.retain(|&r| r != i);
        self.recent.push_front(i);
        self.recent.truncate(MAX_RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_built_in_list() {
        let l = list();
        assert_eq!(l.groups.len(), 9);
        assert!(l.emoji.len() > 1800);
        let up = l.emoji.iter().find(|e| e.name == "thumbs up").unwrap();
        assert_eq!(up.text, "👍");
        assert_eq!(up.tones.unwrap()[0], "👍🏻");
        assert!(l.emoji.iter().all(|e| e.group < l.groups.len()));
    }

    #[test]
    fn matches_word_starts() {
        let l = list();
        let found = |q: &str| search(l, q).into_iter().map(|i| l.emoji[i].name).collect::<Vec<_>>();
        assert!(found("thumb").contains(&"thumbs up"));
        assert!(found("thu up").contains(&"thumbs up"));
        // Not the middle of a word.
        assert!(!found("umbs").contains(&"thumbs up"));
        assert!(found("").is_empty());
    }

    #[test]
    fn name_matches_rank_before_keywords() {
        let l = list();
        let r = search(l, "joy");
        let first = &l.emoji[r[0]];
        assert!(first.name_words.iter().any(|w| w.starts_with("joy")), "{}", first.name);
        // 😂 has "joy" in its name; a keyword-only match must come after every name match.
        let name_end = r.iter().position(|&i| !l.emoji[i].name_words.iter().any(|w| w.starts_with("joy"))).unwrap_or(r.len());
        assert!(r[name_end..].iter().all(|&i| !l.emoji[i].name_words.iter().any(|w| w.starts_with("joy"))));
        // "lol" is only a keyword of 😂.
        assert!(search(l, "lol").iter().any(|&i| l.emoji[i].text == "😂"));
    }

    #[test]
    fn applies_a_skin_tone() {
        let l = list();
        let wave = l.emoji.iter().find(|e| e.name == "waving hand").unwrap();
        assert_eq!(wave.with_tone(None), "👋");
        assert_eq!(wave.with_tone(Some(4)), "👋🏿");
        let cat = l.emoji.iter().find(|e| e.name == "cat face").unwrap();
        assert_eq!(cat.with_tone(Some(2)), "🐱");
    }

    #[test]
    fn recent_moves_repeats_and_drops_the_oldest() {
        let mut m = Memory::default();
        for i in 0..MAX_RECENT + 5 {
            m.picked(i);
        }
        assert_eq!(m.recent.len(), MAX_RECENT);
        assert_eq!(m.recent[0], MAX_RECENT + 4);
        m.picked(10);
        assert_eq!(m.recent[0], 10);
        assert_eq!(m.recent.iter().filter(|&&r| r == 10).count(), 1);
        // Round trip through the state file text.
        m.tone = Some(3);
        let back = Memory::from_text(list(), &m.to_text(list()));
        assert_eq!(back.recent, m.recent);
        assert_eq!(back.tone, Some(3));
    }
}
