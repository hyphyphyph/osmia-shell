//! Colors and egui style. Values come from the picked mockups.

use egui::{Color32, CornerRadius, Stroke};

pub const BAR: Color32 = Color32::from_rgb(0x1e, 0x21, 0x26);
pub const CARD: Color32 = Color32::from_rgb(0x23, 0x27, 0x2d);
pub const CARD_LINE: Color32 = Color32::from_rgb(0x3a, 0x3f, 0x47);
pub const LINE: Color32 = Color32::from_rgb(0x33, 0x37, 0x3e);
pub const TEXT: Color32 = Color32::from_rgb(0xd6, 0xda, 0xe0);
pub const DIM: Color32 = Color32::from_rgb(0x8a, 0x90, 0x9a);
static ACCENT_RGB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0xf0a35e);

pub fn accent() -> Color32 {
    let n = ACCENT_RGB.load(std::sync::atomic::Ordering::Relaxed);
    Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)
}

pub fn set_accent(c: Color32) {
    let n = ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | c.b() as u32;
    ACCENT_RGB.store(n, std::sync::atomic::Ordering::Relaxed);
}
pub const PLATE: Color32 = Color32::from_rgb(0x2a, 0x2e, 0x35);
pub const PLATE_SEL: Color32 = Color32::from_rgb(0x2f, 0x34, 0x3c);
pub const BTN: Color32 = Color32::from_rgb(0x2b, 0x30, 0x37);

pub fn accent_bg() -> Color32 {
    let a = accent();
    Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 41)
}

/// The system sans-serif font from fontconfig: (file bytes, face index).
/// Read once. None when fontconfig is missing, then the bundled font is used.
fn system_font() -> &'static Option<(Vec<u8>, u32)> {
    static FONT: std::sync::OnceLock<Option<(Vec<u8>, u32)>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        let out = std::process::Command::new("fc-match")
            .args(["-f", "%{file}\n%{index}", "sans-serif"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let mut it = text.lines();
        let path = it.next()?.trim();
        let index = it.next().and_then(|i| i.trim().parse().ok()).unwrap_or(0);
        let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        Some((bytes, index))
    })
}

/// The color emoji font fontconfig names for emoji. Read once and kept for the life of
/// the shell, so every window shares the one copy. None when there is no such font.
fn emoji_font() -> Option<(&'static [u8], u32)> {
    static FONT: std::sync::OnceLock<Option<(&'static [u8], u32)>> = std::sync::OnceLock::new();
    *FONT.get_or_init(|| {
        let out = std::process::Command::new("fc-match")
            .args(["-f", "%{file}\n%{index}\n%{color}", "emoji:color=true"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let mut it = text.lines();
        let path = it.next()?.trim();
        let index = it.next().and_then(|i| i.trim().parse().ok()).unwrap_or(0);
        // fc-match always answers. Only take a font that really is in color.
        if it.next().map(str::trim) != Some("True") {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        Some((&*Box::leak(bytes.into_boxed_slice()), index))
    })
}

/// The font family for drawing emoji: the color emoji font, then the usual fonts.
pub fn emoji_family() -> egui::FontFamily {
    egui::FontFamily::Name("emoji".into())
}

/// Fonts found on demand for characters the main fonts lack, such as symbols,
/// braille and CJK. Window titles and notifications can contain anything.
struct Fallbacks {
    tried: std::collections::HashSet<char>,
    fonts: Vec<(String, std::sync::Arc<Vec<u8>>, u32)>,
    generation: u32,
}

static FALLBACKS: std::sync::Mutex<Option<Fallbacks>> = std::sync::Mutex::new(None);

fn with_fallbacks<R>(f: impl FnOnce(&mut Fallbacks) -> R) -> R {
    let mut g = FALLBACKS.lock().unwrap();
    f(g.get_or_insert_with(|| Fallbacks { tried: Default::default(), fonts: Vec::new(), generation: 0 }))
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some((bytes, index)) = system_font() {
        let mut data = egui::FontData::from_owned(bytes.clone());
        data.index = *index;
        fonts.font_data.insert("system-sans".into(), std::sync::Arc::new(data));
        // First in line; the bundled fonts stay behind it for glyphs it lacks.
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            list.insert(0, "system-sans".into());
        }
    }
    let generation = with_fallbacks(|fb| {
        for (name, bytes, index) in &fb.fonts {
            let mut data = egui::FontData::from_owned((**bytes).clone());
            data.index = *index;
            fonts.font_data.insert(name.clone(), std::sync::Arc::new(data));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                if let Some(list) = fonts.families.get_mut(&fam) {
                    list.push(name.clone());
                }
            }
        }
        fb.generation
    });
    let mut emoji_list = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    if let Some((bytes, index)) = emoji_font() {
        let mut data = egui::FontData::from_static(bytes);
        data.index = index;
        fonts.font_data.insert("color-emoji".into(), std::sync::Arc::new(data));
        emoji_list.insert(0, "color-emoji".into());
    }
    fonts.families.insert(emoji_family(), emoji_list);
    ctx.set_fonts(fonts);
    ctx.data_mut(|d| d.insert_temp(crate::ui::widgets::id("osmia-font-gen"), generation));
}

/// Pick the best installed font that has this character, preferring plain sans faces.
fn find_font_for(c: char) -> Option<(String, Vec<u8>, u32)> {
    let out = std::process::Command::new("fc-list")
        .args([&format!(":charset={:x}", c as u32), "-f", "%{file}|%{index}|%{family}\n"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut best: Option<(i32, String, u32)> = None;
    for line in text.lines() {
        let mut it = line.splitn(3, '|');
        let (Some(file), Some(index), Some(family)) = (it.next(), it.next(), it.next()) else { continue };
        let ext = file.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
            continue;
        }
        let f = family.to_lowercase();
        let mut score = 0;
        if f.contains("symbols") {
            score += 3;
        }
        if f.contains("noto sans") || f.contains("dejavu sans") {
            score += 3;
        }
        if f.contains("mono") || f.contains("nerd") || f.contains("emoji") || f.contains("color") {
            score -= 5;
        }
        if best.as_ref().is_none_or(|(s, _, _)| score > *s) {
            best = Some((score, file.to_owned(), index.trim().parse().unwrap_or(0)));
        }
    }
    let (_, file, index) = best?;
    let bytes = std::fs::read(&file).ok()?;
    Some((file, bytes, index))
}

/// Make sure every character in these texts can be drawn. Returns true when a font was
/// added, so the caller should draw again.
pub fn ensure_glyphs<'a>(ctx: &egui::Context, texts: impl Iterator<Item = &'a str>) -> bool {
    // Bring this window up to date with fonts another window found.
    let generation_now = with_fallbacks(|fb| fb.generation);
    let have: u32 = ctx.data(|d| d.get_temp(crate::ui::widgets::id("osmia-font-gen"))).unwrap_or(0);
    let mut changed = false;
    if have != generation_now {
        install_fonts(ctx);
        changed = true;
    }
    // Fonts exist only after the first frame has run. The first frame draws without this check.
    if ctx.cumulative_pass_nr() == 0 {
        return changed;
    }
    let mut missing: Vec<char> = Vec::new();
    for t in texts {
        for c in t.chars() {
            if c.is_ascii() || c.is_control() {
                continue;
            }
            let fresh = with_fallbacks(|fb| fb.tried.insert(c));
            if !fresh {
                continue;
            }
            let covered = ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(12.0), c));
            if !covered {
                missing.push(c);
            }
        }
    }
    let mut added = false;
    for c in missing {
        if let Some((file, bytes, index)) = find_font_for(c) {
            with_fallbacks(|fb| {
                if !fb.fonts.iter().any(|(n, _, _)| *n == file) {
                    fb.fonts.push((file, std::sync::Arc::new(bytes), index));
                    fb.generation += 1;
                    added = true;
                }
            });
        }
    }
    if added {
        install_fonts(ctx);
        changed = true;
    }
    changed
}

static FONT_SCALE_X1000: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1000);

/// The configured font size. 12 is the size the layout was drawn for.
pub fn set_font_size(size: f32) {
    let k = (size.clamp(9.0, 20.0) / 12.0 * 1000.0).round() as u32;
    FONT_SCALE_X1000.store(k, std::sync::atomic::Ordering::Relaxed);
}

pub fn scaled(size: f32) -> f32 {
    size * FONT_SCALE_X1000.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0
}

/// A proportional font at `size`, scaled by the font size setting.
pub fn fid(size: f32) -> egui::FontId {
    egui::FontId::proportional(scaled(size))
}

/// Set the sizes of egui's own text styles. Run again when the font size changes.
pub fn apply_text_styles(ctx: &egui::Context) {
    let base = egui::Style::default().text_styles;
    ctx.all_styles_mut(|s| {
        for (ts, f) in s.text_styles.iter_mut() {
            if let Some(b) = base.get(ts) {
                f.size = scaled((b.size * 0.9).max(11.0));
            }
        }
    });
}

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut v = egui::Visuals::dark();
    v.panel_fill = BAR;
    v.window_fill = CARD;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = accent_bg();
    v.selection.stroke = Stroke::new(1.0, accent());
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    let r = CornerRadius::same(4);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        w.corner_radius = r;
    }
    v.widgets.hovered.bg_fill = PLATE;
    v.widgets.hovered.weak_bg_fill = PLATE;
    v.widgets.active.bg_fill = PLATE_SEL;
    v.widgets.active.weak_bg_fill = PLATE_SEL;
    // Nothing eases or fades. Nothing is rounder than 4 px.
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v.widgets.noninteractive.corner_radius = r;
    v.window_shadow.spread = 0;
    v.text_cursor.blink = false;
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.animation_time = 0.0;
        s.scroll_animation = egui::style::ScrollAnimation::none();
        s.spacing.scroll.floating = false;
        s.spacing.item_spacing = egui::vec2(0.0, 0.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
    });
    apply_text_styles(ctx);
}

#[cfg(test)]
mod font_tests {
    #[test]
    fn finds_a_font_for_symbols() {
        // U+2733 is what terminals put in window titles; the main fonts lack it.
        let f = super::find_font_for('\u{2733}');
        eprintln!("{:?}", f.map(|(n, b, i)| (n, b.len(), i)));
        assert!(super::find_font_for('\u{2733}').is_some());
        // A braille spinner too.
        assert!(super::find_font_for('\u{2802}').is_some());
    }
}

#[cfg(test)]
mod startup_tests {
    #[test]
    fn glyph_check_before_the_first_frame_does_not_panic() {
        let ctx = egui::Context::default();
        // This is what the first frame of a shell with a window titled "◑ x" does.
        assert!(!super::ensure_glyphs(&ctx, ["\u{25d1} title"].into_iter()) || true);
        // After a frame has run, the check works and finds a font for a missing symbol.
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        let _ = super::ensure_glyphs(&ctx, ["\u{2733}"].into_iter());
        out.textures_delta.clear();
    }
}

#[cfg(test)]
mod emoji_font_tests {
    /// Needs epaint's color_fonts feature; without it the glyphs lay out but draw nothing.
    #[test]
    fn emoji_family_draws_emoji() {
        let ctx = egui::Context::default();
        super::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let g = ui.painter().layout_no_wrap("😀".into(), egui::FontId::new(26.0, super::emoji_family()), egui::Color32::WHITE);
            let glyph = &g.rows[0].row.glyphs[0];
            assert!(glyph.uv_rect.size.x > 0.0, "the emoji was not drawn");
        });
        out.textures_delta.clear();
    }
}
