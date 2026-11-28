//! The two-page 2024 character sheet layout, drawn as SVG with HTML
//! overlays for the free-text boxes.
//!
//! Every coordinate is measured from the reference sheet (603 × 774 pt);
//! see `art.rs`. Publisher logos and illustrations are replaced by neutral
//! ornaments.

// Drawing helpers take coordinates as plain numbers.
#![allow(clippy::too_many_arguments)]

use super::art::{
    self, esc, text_width, Anchor, Corner, Corner::*, Font, Svg, BAND, DIVIDER, FIELD, INK, SOFT,
};
use compendium::{signed, Ability, Entry};
use rules::build::FeatureKind;
use rules::sheet::{Proficiency, Sheet, SpellLine};
use rules::Character;

/// One rendered page: SVG art + values, and positioned HTML text boxes.
pub struct Page {
    pub svg: String,
    pub html: String,
}

// ------------------------------------------------------------------ page size

thread_local! {
    /// Height of the page being drawn, in reference units: the reference's
    /// 774 plus the extra length the paper allows (see [`extra_height`]).
    static PAGE_H: std::cell::Cell<f32> = const { std::cell::Cell::new(art::H) };
}

/// Extra page length, in reference units, so the margins are even on all
/// four sides: the boxes nearest the bottom grow by this much. The sheet is
/// scaled to the paper width (see `--u` in style.css).
pub fn extra_height(a4: bool) -> f32 {
    if a4 {
        91.0 // 210 × 297 mm at 0.94 pt per unit
    } else {
        15.0 // 8.5 × 11 in at 0.967 pt per unit
    }
}

fn page_h() -> f32 {
    PAGE_H.with(|h| h.get())
}

// ------------------------------------------------------------------ HTML boxes

fn place(x: f32, y: f32, w: f32, h: f32) -> String {
    format!(
        "left:{:.3}%;top:{:.3}%;width:{:.3}%;height:{:.3}%",
        x / art::W * 100.0,
        y / page_h() * 100.0,
        w / art::W * 100.0,
        h / page_h() * 100.0
    )
}

fn html_box(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    size: f32,
    class: &str,
    inner: &str,
    label: &str,
) -> String {
    format!(
        r#"<div class="rp-box {class}" role="group" aria-label="{}" style="{};font-size:calc(var(--u) * {size:.2})">{inner}</div>"#,
        esc(label),
        place(x, y, w, h)
    )
}

/// A list entry: bold name, optional small tag, optional short description.
#[derive(Clone)]
struct Item {
    name: String,
    tag: String,
    detail: String,
}

/// A one-line description of a feature: its first real sentence, or the
/// names of its parts when it opens with "You gain the following benefits."
fn summary(body: &[Entry]) -> String {
    let lead_in = |t: &str| {
        let l = t.to_lowercase();
        t.trim_end().ends_with(':')
            || l.contains("the following benefit")
            || l.contains("gain the following")
    };
    let text = Entry::plain(body);
    let mut first = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();
    if lead_in(&first) {
        let parts: Vec<String> = body
            .iter()
            .filter_map(|e| match e {
                Entry::Section { name: Some(n), .. } => Some(n.trim_end_matches('.').to_string()),
                _ => None,
            })
            .collect();
        first = if parts.is_empty() {
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .nth(1)
                .unwrap_or("")
                .to_string()
        } else {
            parts.join("; ")
        };
    }
    let end = first.find(". ").map(|i| i + 1).unwrap_or(first.len());
    let mut s = first[..end].to_string();
    if s.chars().count() > 150 {
        let cut: String = s.chars().take(147).collect();
        s = format!("{}…", cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut));
    }
    s
}

/// Estimated height of `items` set at `size` in columns of width `w`.
fn list_height(items: &[Item], size: f32, w: f32, details: bool, cols: usize) -> f32 {
    let line = size * 1.17;
    let mut lines = 0.0;
    for it in items {
        let mut t = it.name.clone();
        if !it.tag.is_empty() {
            t.push_str("  ");
            t.push_str(&it.tag);
        }
        if details && !it.detail.is_empty() {
            t.push_str(" — ");
            t.push_str(&it.detail);
        }
        lines += (text_width(&t, size) / w).ceil().max(1.0) + 0.22;
    }
    // Column breaks waste up to one entry per column.
    lines * line / cols as f32 + if cols > 1 { line * 1.5 } else { 0.0 }
}

/// Chooses the largest type size (and whether descriptions fit) for a list
/// box; returns how many items can be shown.
fn fit_list(items: &[Item], w: f32, h: f32, cols: usize, max: f32) -> (f32, bool, usize) {
    let sizes = [7.5, 7.25, 7.0, 6.75, 6.5, 6.25, 6.0, 5.75, 5.5];
    for details in [true, false] {
        for &s in sizes.iter().filter(|s| **s <= max) {
            if list_height(items, s, w, details, cols) <= h {
                return (s, details, items.len());
            }
        }
    }
    let mut n = items.len();
    while n > 0 && list_height(&items[..n], 5.5, w, false, cols) + 8.0 > h {
        n -= 1;
    }
    (5.5, false, n)
}

fn list_html(items: &[Item], details: bool, more: usize, more_note: &str) -> String {
    let mut o = String::new();
    for it in items {
        o.push_str("<p><b>");
        o.push_str(&esc(&it.name));
        o.push_str("</b>");
        if !it.tag.is_empty() {
            o.push_str(&format!(r#" <span class="tag">{}</span>"#, esc(&it.tag)));
        }
        if details && !it.detail.is_empty() {
            o.push_str(&format!(r#" <span class="d">— {}</span>"#, esc(&it.detail)));
        }
        o.push_str("</p>");
    }
    if more > 0 {
        o.push_str(&format!(
            r#"<p class="more">+{more} more — {}</p>"#,
            esc(more_note)
        ));
    }
    o
}

/// A fitted list box. Items that don't fit are counted in a closing
/// "+N more — {more_note}" line.
#[allow(clippy::too_many_arguments)]
fn list_box(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    cols: usize,
    gap: f32,
    items: Vec<Item>,
    label: &str,
    more_note: &str,
) -> String {
    let col_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    let (size, details, n) = fit_list(&items, col_w, h, cols, 7.5);
    let more = items.len() - n;
    let inner = list_html(&items[..n], details, more, more_note);
    let class = if cols > 1 { "list cols" } else { "list" };
    let mut html = html_box(x, y, w, h, size, class, &inner, label);
    if cols > 1 {
        html = html.replacen(
            "style=\"",
            &format!("style=\"column-gap:calc(var(--u) * {gap});"),
            1,
        );
    }
    html
}

/// Paragraph text fitted into a box: (label, text) pairs. Shrinks to a
/// minimum size; anything still too long is cut with "…".
fn text_box(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    paras: &[(&str, String)],
    label: &str,
) -> String {
    let paras: Vec<(&str, String)> = paras
        .iter()
        .filter(|(_, t)| !t.trim().is_empty())
        .map(|(l, t)| (*l, t.trim().to_string()))
        .collect();
    let height = |size: f32, ps: &[(&str, String)]| -> f32 {
        ps.iter()
            .map(|(l, t)| {
                let lines: f32 = t
                    .lines()
                    .map(|ln| (text_width(&format!("{l} {ln}"), size) / w).ceil().max(1.0))
                    .sum();
                (lines + 0.25) * size * 1.17
            })
            .sum()
    };
    let render = |ps: &[(&str, String)]| -> String {
        ps.iter()
            .map(|(l, t)| {
                let lab = if l.is_empty() {
                    String::new()
                } else {
                    format!("<b>{}</b> ", esc(l))
                };
                format!("<p>{lab}{}</p>", esc(t).replace('\n', "<br>"))
            })
            .collect()
    };
    for s in [8.0, 7.5, 7.0, 6.5, 6.0, 5.5] {
        if height(s, &paras) <= h {
            return html_box(x, y, w, h, s, "text", &render(&paras), label);
        }
    }
    // Keep whole paragraphs that fit, then as many words of the next as fit.
    let mut n = paras.len();
    while n > 0 && height(5.5, &paras[..n]) > h {
        n -= 1;
    }
    let mut kept: Vec<(&str, String)> = paras[..n].to_vec();
    if let Some((l, t)) = paras.get(n) {
        let words: Vec<&str> = t.split_whitespace().collect();
        let mut k = words.len();
        while k > 0 {
            let mut trial = kept.clone();
            trial.push((l, format!("{}…", words[..k].join(" "))));
            if height(5.5, &trial) <= h {
                kept = trial;
                break;
            }
            k -= 1;
        }
    }
    html_box(x, y, w, h, 5.5, "text", &render(&kept), label)
}

// ------------------------------------------------------------------ helpers

fn prof_fill(p: Proficiency) -> u8 {
    match p {
        Proficiency::None => 0,
        Proficiency::Half => 1,
        Proficiency::Proficient => 2,
        Proficiency::Expertise => 3,
    }
}

fn joined(v: &[String]) -> String {
    v.join(", ")
}

/// Decorative "curtain" lines that hang below a box title, as on the
/// reference: an inner pillar and two crossing drapes on each side.
fn curtains(g: &mut Svg, x: f32, y: f32, w: f32, bottom: f32) {
    let h = bottom - y;
    for (sx, dir) in [(x, 1.0), (x + w, -1.0)] {
        let px = |dx: f32| sx + dx * dir;
        let d = format!(
            "M{} {y}C{} {} {} {} {} {bottom}M{} {y}C{} {} {} {} {} {bottom}",
            px(16.0),
            px(9.0),
            y + h * 0.45,
            px(15.0),
            y + h * 0.7,
            px(22.0),
            px(9.5),
            px(15.5),
            y + h * 0.35,
            px(19.5),
            y + h * 0.65,
            px(13.5),
        );
        g.path(&d, SOFT, 0.6, "none");
        g.vline(px(7.5), y + 2.5, h - 3.0, 0.6, SOFT);
    }
}

/// A small stat box (Initiative, Speed, Size, Passive Perception).
fn stat_box(
    g: &mut Svg,
    x: f32,
    w: f32,
    title: &str,
    title_x: f32,
    title_len: f32,
    rule: (f32, f32),
) {
    g.frame(
        x,
        112.1,
        w,
        40.8,
        [Notch(4.5), Square, Chamfer(3.0), Chamfer(3.0)],
    );
    g.knob(x + 0.6, 112.6);
    g.text(
        title_x,
        122.5,
        Font::Head,
        Anchor::Start,
        Some(title_len),
        title,
    );
    g.title_rule(rule.0, 127.1, rule.1);
    curtains(g, x, 127.6, w, 152.4);
}

/// Six ability blocks: (ability, inner x, inner y, inner height).
const ABILITY_BLOCKS: [(Ability, f32, f32, f32); 6] = [
    (Ability::Str, 12.2, 191.5, 104.6),
    (Ability::Dex, 12.2, 309.3, 132.7),
    (Ability::Con, 12.2, 455.2, 85.0),
    (Ability::Int, 118.4, 114.2, 160.8),
    (Ability::Wis, 118.4, 288.1, 160.8),
    (Ability::Cha, 118.4, 462.0, 146.7),
];

fn ability_block(g: &mut Svg, s: &Sheet, ab: Ability, ix: f32, iy: f32, ih: f32) {
    let iw = 90.9;
    let Some(a) = s.abilities.iter().find(|a| a.ability == ab) else {
        return;
    };
    // Frame, side pillars and title.
    g.frame(
        ix,
        iy,
        iw,
        ih,
        [Notch(6.0), Notch(6.0), Chamfer(4.0), Chamfer(4.0)],
    );
    g.vline(ix - 4.4, iy + 9.0, ih - 13.0, 0.6, SOFT);
    g.vline(ix + iw + 4.4, iy + 9.0, ih - 13.0, 0.6, SOFT);
    let (tx, tw) = match ab {
        Ability::Str => (36.7, 41.2),
        Ability::Dex => (36.9, 40.6),
        Ability::Con => (27.8, 59.0),
        Ability::Int => (136.1, 54.8),
        Ability::Wis => (146.8, 33.4),
        Ability::Cha => (143.3, 40.3),
    };
    g.text(
        tx,
        iy + 8.3,
        Font::Head,
        Anchor::Start,
        Some(tw),
        &ab.name().to_uppercase(),
    );
    // Curtains framing the score emblem.
    let d = format!(
        "M{} {}C{} {} {} {} {} {}M{} {}C{} {} {} {} {} {}",
        ix + 3.0,
        iy + 9.0,
        ix + 13.0,
        iy + 22.0,
        ix + 3.0,
        iy + 40.0,
        ix + 11.0,
        iy + 61.0,
        ix + iw - 3.0,
        iy + 9.0,
        ix + iw - 13.0,
        iy + 22.0,
        ix + iw - 3.0,
        iy + 40.0,
        ix + iw - 11.0,
        iy + 61.0
    );
    g.path(&d, SOFT, 0.6, "none");
    // Score tab (folder shape with a doubled right edge), then modifier circle.
    let (t0, t1) = (iy + 22.5, iy + 46.0);
    let tab = format!(
        "M{} {t0}H{}L{} {}H{}V{t1}H{}Z",
        ix + 45.0,
        ix + 60.8,
        ix + 63.3,
        iy + 25.5,
        ix + 75.8,
        ix + 45.0
    );
    g.path(&tab, BAND, 1.1, "#fff");
    g.path(
        &format!(
            "M{} {}L{} {}V{t1}H{}",
            ix + 75.8,
            iy + 25.5,
            ix + 78.8,
            iy + 28.0,
            ix + 75.8
        ),
        BAND,
        1.0,
        "none",
    );
    let (cx, cy, r) = (ix + 30.9, iy + 32.9, 19.4);
    g.circle(cx, cy, r, BAND, 2.0, "#fff");
    g.circle(cx, cy, r - 1.9, "#c9c7c7", 0.5, "none");
    g.path(
        &format!(
            "M{} {}L{cx} {}L{} {}",
            cx - 3.2,
            cy + r - 0.6,
            cy + r - 5.0,
            cx + 3.2,
            cy + r - 0.6
        ),
        BAND,
        0.8,
        "#fff",
    );
    g.text(
        ix + 16.0,
        iy + 57.3,
        Font::LabelSmall,
        Anchor::Start,
        Some(30.7),
        "modifier",
    );
    g.text(
        ix + 51.8,
        iy + 52.2,
        Font::LabelSmall,
        Anchor::Start,
        Some(19.6),
        "score",
    );
    g.value(
        cx,
        cy + 6.0,
        30.0,
        17.0,
        Anchor::Middle,
        &signed(a.modifier),
    );
    g.value(
        ix + 60.4,
        iy + 38.6,
        27.0,
        10.5,
        Anchor::Middle,
        &a.score.to_string(),
    );

    // Saving throw row.
    g.hline(ix, iy + 62.3, iw, 0.8, BAND);
    g.bubble(ix + 9.4, iy + 73.9, if a.save_prof { 2 } else { 0 });
    g.field(ix + 15.3, iy + 77.8, 17.1);
    g.value(
        ix + 23.85,
        iy + 76.4,
        17.0,
        7.5,
        Anchor::Middle,
        &signed(a.save),
    );
    g.text(
        ix + 35.7,
        iy + 76.3,
        Font::TextBold,
        Anchor::Start,
        Some(40.6),
        "Saving Throw",
    );

    // Skills.
    let skills: Vec<_> = s
        .skills
        .iter()
        .filter(|k| k.skill.ability() == ab)
        .collect();
    if !skills.is_empty() {
        g.hline(ix, iy + 83.0, iw, 0.8, BAND);
    }
    for (i, k) in skills.iter().enumerate() {
        let dy = i as f32 * 14.0;
        g.bubble(ix + 9.4, iy + 93.5 + dy, prof_fill(k.prof));
        g.field(ix + 15.3, iy + 97.4 + dy, 17.1);
        g.value(
            ix + 23.85,
            iy + 96.0 + dy,
            17.0,
            7.5,
            Anchor::Middle,
            &signed(k.bonus),
        );
        g.text(
            ix + 34.7,
            iy + 96.3 + dy,
            Font::Text,
            Anchor::Start,
            skill_width(k.skill.name()),
            k.skill.name(),
        );
    }
}

/// Printed widths of the skill names on the reference, so ours line up.
fn skill_width(name: &str) -> Option<f32> {
    Some(match name {
        "Athletics" => 27.3,
        "Acrobatics" => 33.0,
        "Sleight of Hand" => 48.4,
        "Stealth" => 21.6,
        "Arcana" => 21.7,
        "History" => 23.2,
        "Investigation" => 40.5,
        "Nature" => 21.7,
        "Religion" => 25.4,
        "Animal Handling" => 51.2,
        "Insight" => 21.6,
        "Medicine" => 29.0,
        "Perception" => 33.6,
        "Survival" => 24.8,
        "Deception" => 32.4,
        "Intimidation" => 38.6,
        "Performance" => 40.2,
        "Persuasion" => 34.5,
        _ => return None,
    })
}

// ------------------------------------------------------------------ page 1

pub fn page1(ch: &Character, s: &Sheet, ext: f32) -> Page {
    PAGE_H.with(|h| h.set(art::H + ext));
    let mut g = Svg::default();
    let mut html = String::new();
    let d = &ch.details;

    // Header: name, background, class, species, subclass.
    g.frame(
        12.1,
        12.0,
        246.2,
        72.0,
        [Notch(8.0), Square, Square, Square],
    );
    g.knob(13.6, 13.6);
    g.path(
        "M16.5 21C19.5 24 21.6 27 21.8 31M9.6 30C8.6 26 9.2 22 11 19.5M9.6 30V82",
        SOFT,
        0.6,
        "none",
    );
    g.field(24.9, 29.9, 226.5);
    g.field(24.9, 51.4, 118.3);
    g.field(148.8, 51.4, 98.0);
    g.field(24.8, 73.0, 118.3);
    g.field(148.6, 73.0, 107.9);
    g.text(
        24.9,
        36.3,
        Font::Label,
        Anchor::Start,
        Some(66.3),
        "character name",
    );
    g.text(
        24.9,
        57.8,
        Font::Label,
        Anchor::Start,
        Some(51.5),
        "background",
    );
    g.text(148.8, 57.8, Font::Label, Anchor::Start, Some(21.5), "class");
    g.text(
        24.8,
        79.3,
        Font::Label,
        Anchor::Start,
        Some(28.8),
        "species",
    );
    g.text(
        148.6,
        79.3,
        Font::Label,
        Anchor::Start,
        Some(35.8),
        "subclass",
    );
    let multi = s.classes.len() > 1;
    let class = if multi {
        s.classes
            .iter()
            .map(|c| format!("{} {}", c.name, c.level))
            .collect::<Vec<_>>()
            .join(" / ")
    } else {
        s.classes
            .first()
            .map(|c| c.name.clone())
            .unwrap_or_default()
    };
    let subclass = s
        .classes
        .iter()
        .filter_map(|c| c.subclass.clone())
        .collect::<Vec<_>>()
        .join(" / ");
    g.value(25.4, 27.4, 225.0, 14.0, Anchor::Start, &s.name);
    g.value(25.4, 49.2, 117.0, 9.5, Anchor::Start, &s.background);
    g.value(149.3, 49.2, 97.0, 9.5, Anchor::Start, &class);
    g.value(25.4, 70.8, 117.0, 9.5, Anchor::Start, &s.species);
    g.value(149.1, 70.8, 107.0, 9.5, Anchor::Start, &subclass);

    // Level & XP oval.
    g.ellipse(276.4, 47.7, 29.0, 34.6, BAND, 2.2, "#fff");
    g.ellipse(276.4, 47.7, 26.9, 32.5, "#c9c7c7", 0.5, "none");
    g.path("M276.4 17.5C268 24 256 31 255.5 45M276.4 17.5C284.8 24 296.8 31 297.3 45M273.6 19.8L276.4 15.6L279.2 19.8", SOFT, 0.6, "none");
    g.field(262.9, 46.4, 26.1);
    g.field(258.7, 68.0, 35.5);
    g.text(266.0, 52.8, Font::Label, Anchor::Start, Some(20.7), "level");
    g.text(272.5, 74.3, Font::Label, Anchor::Start, Some(8.9), "xp");
    g.value(
        275.95,
        44.4,
        26.0,
        14.0,
        Anchor::Middle,
        &s.level.to_string(),
    );
    g.value(
        276.45,
        66.0,
        34.0,
        8.0,
        Anchor::Middle,
        &if d.xp > 0 {
            d.xp.to_string()
        } else {
            String::new()
        },
    );

    // Armor class shield.
    let shield = "M313.6 16.2Q313.6 12 317.4 12H361.4Q365.2 12 365.2 16.2V50C365.2 64 354 74.5 339.4 84.6C324.8 74.5 313.6 64 313.6 50Z";
    g.path(shield, BAND, 2.4, "#fff");
    g.path("M316.6 18Q316.6 15 319.6 15H359.2Q362.2 15 362.2 18V50C362.2 62.4 352.2 71.6 339.4 80.9C326.6 71.6 316.6 62.4 316.6 50Z", "#c9c7c7", 0.5, "none");
    g.text(325.2, 22.8, Font::Head, Anchor::Start, Some(30.0), "ARMOR");
    g.text(327.8, 30.8, Font::Head, Anchor::Start, Some(22.9), "CLASS");
    g.text(
        325.9,
        69.3,
        Font::Label,
        Anchor::Start,
        Some(26.9),
        "shield",
    );
    g.diamond(339.35, 75.65, 4.14, 1.0, s.shield);
    g.value(
        339.4,
        56.0,
        44.0,
        22.0,
        Anchor::Middle,
        &s.armor_class.to_string(),
    );

    // Hit points, hit dice, death saves.
    g.frame(
        373.8,
        12.0,
        217.2,
        72.0,
        [Square, Notch(8.0), Square, Square],
    );
    g.knob(589.4, 13.6);
    g.path(
        "M593.4 30C594.4 26 593.8 22 592 19.5M593.4 30V82",
        SOFT,
        0.6,
        "none",
    );
    g.vline(486.7, 12.9, 70.4, 1.0, INK);
    g.vline(535.5, 12.9, 70.4, 1.0, INK);
    g.vline(432.3, 26.3, 51.0, 1.0, SOFT);
    g.text(
        407.8,
        22.8,
        Font::Head,
        Anchor::Start,
        Some(44.3),
        "HIT POINTS",
    );
    g.text(
        492.3,
        22.8,
        Font::Head,
        Anchor::Start,
        Some(33.5),
        "HIT DICE",
    );
    g.text(541.6, 22.8, Font::Head, Anchor::Start, Some(25.7), "DEATH");
    g.text(541.6, 30.8, Font::Head, Anchor::Start, Some(22.9), "SAVES");
    g.field(438.3, 49.6, 42.7);
    g.field(438.3, 71.2, 42.7);
    g.field(379.2, 71.2, 47.6);
    g.field(492.3, 49.6, 37.5);
    g.field(492.3, 71.2, 37.5);
    g.text(
        379.2,
        77.5,
        Font::Label,
        Anchor::Start,
        Some(33.7),
        "current",
    );
    g.text(438.3, 56.0, Font::Label, Anchor::Start, Some(19.6), "temp");
    g.text(438.3, 77.5, Font::Label, Anchor::Start, Some(16.0), "max");
    g.text(492.3, 56.0, Font::Label, Anchor::Start, Some(22.9), "spent");
    g.text(492.3, 77.5, Font::Label, Anchor::Start, Some(16.0), "max");
    g.text(
        542.0,
        56.0,
        Font::Label,
        Anchor::Start,
        Some(40.1),
        "successes",
    );
    g.text(
        542.0,
        77.5,
        Font::Label,
        Anchor::Start,
        Some(33.7),
        "failures",
    );
    for y in [44.2, 65.8] {
        g.raw(&format!(
            r#"<rect x="539" y="{y}" width="42.6" height="1.7" fill="url(#fade)"/>"#
        ));
    }
    for y in [45.0, 66.6] {
        for x in [545.85, 556.85, 567.85] {
            g.diamond(x, y, 4.14, 1.0, false);
        }
    }
    g.value(
        459.65,
        69.0,
        41.0,
        11.0,
        Anchor::Middle,
        &s.hp_max.to_string(),
    );
    g.value(511.05, 69.0, 37.0, 9.0, Anchor::Middle, &s.hit_dice);

    // Divider strip where the reference has its logo: a neutral ornament.
    for (x0, x1, dir) in [(13.0_f32, 222.0_f32, 1.0_f32), (590.0, 381.0, -1.0)] {
        let len = (x1 - x0).abs();
        let xs = x0.min(x1);
        g.hline(xs, 100.6, len, 1.6, BAND);
        g.hline(xs + 4.0, 96.6, len - 8.0, 0.5, "#c9c7c7");
        g.path(
            &format!(
                "M{x0} 100.6C{} 100.6 {} 96 {} 91",
                x0 + 2.0 * dir,
                x0 + 1.0 * dir,
                x0 + 1.0 * dir
            ),
            BAND,
            1.4,
            "none",
        );
        g.path(
            &format!(
                "M{x1} 100.6L{} 93.5L{} 93.5",
                x1 + 2.5 * dir,
                x1 + 6.5 * dir
            ),
            BAND,
            1.2,
            "none",
        );
    }
    g.raw(r##"<path d="M301.5 85.2C302.6 92 303.8 93.2 310.6 94.4C303.8 95.6 302.6 96.8 301.5 103.6C300.4 96.8 299.2 95.6 292.4 94.4C299.2 93.2 300.4 92 301.5 85.2Z" fill="#b2b1b1"/>"##);
    g.diamond(301.5, 94.4, 3.0, 0.9, false);
    for (x, a) in [(281.0, 2.6), (322.0, 2.6), (266.5, 1.9), (336.5, 1.9)] {
        g.diamond(x, 94.4, a, 0.8, false);
    }
    g.hline(229.0, 94.4, 33.0, 0.5, FIELD);
    g.hline(341.0, 94.4, 33.0, 0.5, FIELD);

    // Proficiency bonus.
    g.frame(
        12.2,
        112.1,
        90.9,
        66.3,
        [Notch(6.0), Chamfer(4.0), Chamfer(4.0), Chamfer(4.0)],
    );
    g.knob(12.4, 112.3);
    g.text(
        17.4,
        124.4,
        Font::Head,
        Anchor::Start,
        Some(80.6),
        "PROFICIENCY BONUS",
    );
    g.title_rule(17.8, 130.3, 80.1);
    g.path("M22 130.8C30 134 36.5 138 39 147M93.4 130.8C85.4 134 78.9 138 76.4 147M17 136V172M98.4 136V172", SOFT, 0.6, "none");
    g.path(
        "M40 130.6V168.5Q40 172.5 44 172.5H72.5Q76.5 172.5 76.5 168.5V130.6",
        BAND,
        1.5,
        "#fff",
    );
    g.path(
        "M42.6 130.6V167.4Q42.6 170 45.2 170H71.3Q73.9 170 73.9 167.4V130.6",
        "#c9c7c7",
        0.5,
        "none",
    );
    g.value(
        58.25,
        158.6,
        32.0,
        18.0,
        Anchor::Middle,
        &signed(s.proficiency_bonus),
    );

    // Abilities, saves and skills.
    for (ab, x, y, h) in ABILITY_BLOCKS {
        ability_block(&mut g, s, ab, x, y, h);
    }

    // Heroic inspiration.
    g.frame(
        12.2,
        551.4,
        90.9,
        57.3,
        [Notch(6.0), Square, Round(18.0), Round(18.0)],
    );
    g.knob(12.4, 551.6);
    g.text(43.0, 563.6, Font::Head, Anchor::Start, Some(29.5), "HEROIC");
    g.text(
        33.0,
        571.6,
        Font::Head,
        Anchor::Start,
        Some(49.5),
        "INSPIRATION",
    );
    g.title_rule(17.6, 577.6, 80.1);
    g.circle(57.6, 592.0, 18.5, BAND, 1.6, "#fff");
    g.path("M43.7 578V600M71.5 578V600M20 580C30 584 36 590 39.5 599M95.2 580C85.2 584 79.2 590 75.7 599", SOFT, 0.6, "none");
    g.raw(r##"<path d="M66.78 591.92C60.09 593.31 58.96 594.44 57.56 601.13C56.17 594.44 55.04 593.31 48.35 591.92C55.04 590.52 56.17 589.39 57.56 582.7C58.96 589.39 60.09 590.52 66.78 591.92Z" fill="#b2b1b1"/>"##);
    g.path("M53.28 596.2C54.33 595.14 54.33 588.67 53.28 587.62C54.33 588.67 60.8 588.67 61.86 587.62C60.8 588.67 60.8 595.14 61.86 596.2C60.8 595.14 54.33 595.14 53.28 596.2Z", INK, 0.86, "#fff");

    // Initiative, speed, size, passive perception.
    stat_box(
        &mut g,
        222.4,
        80.2,
        "INITIATIVE",
        242.9,
        39.4,
        (226.5, 72.1),
    );
    stat_box(&mut g, 313.7, 86.4, "SPEED", 344.3, 23.8, (317.8, 78.2));
    stat_box(&mut g, 411.1, 80.2, "SIZE", 443.0, 16.4, (415.2, 72.1));
    stat_box(
        &mut g,
        502.3,
        88.6,
        "PASSIVE PERCEPTION",
        507.5,
        78.3,
        (506.5, 80.4),
    );
    let extra_speed: Vec<String> = [
        ("Fly", s.speed.fly),
        ("Swim", s.speed.swim),
        ("Climb", s.speed.climb),
        ("Burrow", s.speed.burrow),
    ]
    .into_iter()
    .filter(|(_, v)| *v > 0)
    .map(|(n, v)| format!("{n} {v}"))
    .collect();
    g.value(
        262.5,
        146.0,
        60.0,
        16.0,
        Anchor::Middle,
        &signed(s.initiative),
    );
    if extra_speed.is_empty() {
        g.value(
            356.9,
            146.0,
            64.0,
            14.0,
            Anchor::Middle,
            &format!("{} ft.", s.speed.walk),
        );
    } else {
        g.value(
            356.9,
            142.5,
            64.0,
            13.0,
            Anchor::Middle,
            &format!("{} ft.", s.speed.walk),
        );
        g.value(
            356.9,
            150.0,
            66.0,
            6.0,
            Anchor::Middle,
            &extra_speed.join(" · "),
        );
    }
    g.value(451.2, 145.5, 60.0, 12.0, Anchor::Middle, &s.size);
    g.value(
        546.6,
        146.0,
        60.0,
        16.0,
        Anchor::Middle,
        &s.passive_perception.to_string(),
    );

    // Weapons & damage cantrips.
    g.frame(
        222.6,
        163.9,
        368.4,
        114.8,
        [Chamfer(6.0), Square, Square, Square],
    );
    g.text(
        346.4,
        176.1,
        Font::Head,
        Anchor::Start,
        Some(121.3),
        "WEAPONS & DAMAGE CANTRIPS",
    );
    g.title_rule(226.7, 182.2, 360.1);
    // Two lines per attack: the roll on top, what to remember below.
    const COLS: [(f32, f32, &str); 7] = [
        (228.2, 84.0, "Name"),
        (314.5, 26.0, "Use"),
        (343.0, 23.0, "Abil."),
        (369.5, 52.0, "Range"),
        (424.0, 36.0, "Atk / DC"),
        (462.5, 48.0, "Damage"),
        (513.0, 72.5, "Type"),
    ];
    for (x, w, t) in COLS {
        g.text(x, 194.3, Font::Column, Anchor::Start, None, t);
        g.hline(x, 195.4, w, 0.25, FIELD);
    }
    // Four attacks on the page (the Custom step chooses which).
    let rows = [212.6, 232.1, 251.5, 271.0];
    // Never smaller than these (in sheet units, ~1pt): long text is cut with "…".
    const MIN_VALUE: f32 = 6.6;
    const MIN_NOTE: f32 = 5.6;
    for (i, y) in rows.iter().enumerate() {
        g.field(228.2, *y, 357.3);
        for x in [313.3, 341.8, 368.3, 422.8, 461.3, 511.8] {
            g.vline(x, y - 17.2, 9.6, 0.5, DIVIDER);
        }
        let Some(a) = s.attacks.get(i) else { continue };
        let bl = y - 9.4;
        g.value_min(228.6, bl, 83.0, 7.8, MIN_VALUE, Anchor::Start, &a.name);
        if !a.custom {
            let used = match a.time.as_str() {
                "Bonus Action" => "Bonus",
                other => other,
            };
            g.value_min(314.9, bl, 25.5, 6.6, MIN_VALUE, Anchor::Start, used);
            g.diamond(346.8, bl - 2.4, 2.1, 0.5, a.proficient);
            g.value_min(350.8, bl, 16.0, 6.6, MIN_VALUE, Anchor::Start, a.ability.map(|x| x.abbr()).unwrap_or(""));
            g.value_min(369.9, bl, 51.5, 6.8, MIN_VALUE, Anchor::Start, &a.range);
            g.value_min(442.0, bl, 36.0, 7.6, MIN_VALUE, Anchor::Middle, &a.to_hit());
            g.value_min(462.9, bl, 47.5, 7.6, MIN_VALUE, Anchor::Start, &a.dice);
            g.value_min(513.4, bl, 72.0, 7.2, MIN_VALUE, Anchor::Start, &a.damage_type);
        }
        g.note(228.6, y - 2.2, 356.5, 6.2, MIN_NOTE, &a.notes);
    }

    // Actions: what the character can do with each kind of action.
    g.frame(222.6, 289.7, 368.4, 99.3, [Chamfer(6.0), Square, Square, Square]);
    g.text(406.8, 301.9, Font::Head, Anchor::Middle, None, "ACTIONS");
    g.title_rule(226.7, 308.1, 360.1);
    g.text(586.0, 301.9, Font::Column, Anchor::End, None, "spells in italics");
    let a = &s.actions;
    let col_w = 116.0;
    // Damage cantrips already in the weapons rows aren't repeated here.
    let shown_attacks: Vec<&str> = s.attacks.iter().take(4).map(|a| a.name.as_str()).collect();
    for (i, (title, list)) in [("Actions", &a.action), ("Bonus Actions", &a.bonus), ("Reactions", &a.reaction)].into_iter().enumerate() {
        let x = 228.2 + i as f32 * (col_w + 5.0);
        if i > 0 {
            g.vline(x - 2.5, 312.0, 72.5, 0.5, DIVIDER);
        }
        g.text(x, 316.6, Font::Column, Anchor::Start, None, title);
        g.hline(x, 317.7, col_w - 2.0, 0.25, FIELD);
        let entries: Vec<&rules::play::ActionEntry> =
            list.iter().filter(|e| e.spell.is_none() || !shown_attacks.contains(&e.name.as_str())).collect();
        html.push_str(&action_column(x, 319.5, col_w - 2.0, 66.5, &entries, title));
    }

    // Class features and species traits & feats, side by side to the bottom.
    // Class options (maneuvers, invocations…) sit with class features unless a
    // feat granted them. Repeats (Indomitable at 9/13/17, several ASIs) merge.
    let in_feats_box = |f: &rules::build::FeatureEntry| f.kind != FeatureKind::Species && s.in_traits_box(f);
    let edits = &ch.sheet;
    use rules::play::section;
    let feature_items = |pick: &dyn Fn(&rules::build::FeatureEntry) -> bool, count: bool| -> Vec<Item> {
        let mut out: Vec<(Item, (usize, bool))> = Vec::new();
        for f in s.features.iter().filter(|f| pick(f)) {
            // Merge repeats, but never a subclass feature into a class feature
            // of the same name (a legacy oath's own Channel Divinity options).
            let sub = f.origin.contains('(');
            if let Some(e) = out.iter_mut().find(|(i, s2)| i.name == f.feature.name && s2.1 == sub) {
                e.1 .0 += 1;
                continue;
            }
            let item = Item {
                name: f.feature.name.clone(),
                tag: String::new(),
                detail: summary(&f.feature.body),
            };
            out.push((item, (1, sub)));
        }
        out.into_iter()
            .map(|(mut i, (n, _))| {
                if count && n > 1 {
                    i.name = format!("{} ×{n}", i.name);
                }
                i
            })
            .collect()
    };
    let top = 400.0;
    let height = 761.9 + ext - top;

    g.frame(222.6, top, 200.0, height, [Chamfer(5.0), Square, Square, Square]);
    g.text(322.6, top + 12.2, Font::Head, Anchor::Middle, None, "CLASS FEATURES");
    g.grid(222.6, top + 18.4, 200.0, height - 18.4);
    g.title_rule(226.7, top + 18.4, 191.8);
    // The player's edits: hidden entries go, pinned ones and their own
    // entries come first.
    let edited = |sec: &str, items: Vec<Item>| -> Vec<Item> {
        let base = |i: &Item| i.name.split(" ×").next().unwrap_or("").to_string();
        let items = items.into_iter().map(|i| (i, true)).collect();
        let mut out = edits.arrange(sec, items, |i: &Item| i.name.split(" ×").next().unwrap_or(""));
        let pinned = out.iter().take_while(|i| edits.state(sec, &base(i)) == Some(true)).count();
        let own = edits.added(sec).iter().map(|x| Item { name: x.name.clone(), tag: String::new(), detail: x.detail.clone() });
        out.splice(pinned..pinned, own);
        out
    };
    let class_items = edited(
        section::CLASS_FEATURES,
        feature_items(&|f| matches!(f.kind, FeatureKind::Class | FeatureKind::Other) && !in_feats_box(f), false),
    );
    html.push_str(&list_box(228.0, top + 21.5, 189.4, height - 24.5, 1, 0.0, class_items, "Class features", "see the Features & Traits pages"));

    g.frame(433.5, top, 157.5, height, [Square, Chamfer(5.0), Square, Square]);
    g.text(512.25, top + 12.2, Font::Head, Anchor::Middle, None, "SPECIES TRAITS & FEATS");
    g.grid(433.5, top + 18.4, 157.5, height - 18.4);
    g.title_rule(437.6, top + 18.4, 149.3);
    let mut traits: Vec<Item> = feature_items(&|f| f.kind == FeatureKind::Species, false)
        .into_iter()
        .map(|i| Item { tag: "species".into(), ..i })
        .collect();
    let defenses = [
        ("Resistances", &s.resistances),
        ("Immunities", &s.immunities),
        ("Condition immunities", &s.condition_immunities),
        ("Vulnerabilities", &s.vulnerabilities),
        ("Senses", &s.senses),
    ];
    for (label, v) in defenses {
        if !v.is_empty() {
            traits.push(Item {
                name: label.to_string(),
                tag: String::new(),
                detail: joined(v),
            });
        }
    }
    traits.extend(feature_items(&|f| in_feats_box(f), true));
    let traits = edited(section::TRAITS, traits);
    html.push_str(&list_box(438.8, top + 21.5, 147.0, height - 24.5, 1, 0.0, traits, "Species traits and feats", "see the Features & Traits pages"));

    // Equipment training & proficiencies, with languages.
    g.frame(
        12.0,
        619.5,
        199.6,
        142.4 + ext,
        [Square, Chamfer(5.0), Square, Notch(9.0)],
    );
    g.knob(13.4, 760.4 + ext);
    // Weapons, tools and languages split any extra length.
    let e3 = ext / 3.0;
    g.text(
        34.4,
        631.7,
        Font::Head,
        Anchor::Start,
        Some(154.3),
        "EQUIPMENT TRAINING & PROFICIENCIES",
    );
    g.grid(12.0, 637.9, 199.6, 124.0 + ext);
    g.title_rule(16.0, 637.9, 191.4);
    g.text(16.0, 647.8, Font::Label, Anchor::Start, Some(26.6), "armor");
    g.text(16.0, 654.2, Font::Label, Anchor::Start, Some(36.5), "training");
    let has = |p: &str| {
        s.armor_profs
            .iter()
            .any(|a| a.eq_ignore_ascii_case(p) || a.to_lowercase().starts_with(&p.to_lowercase()))
    };
    for (x, tx, w, t, key) in [
        (63.15, 68.9, 15.9, "Light", "light"),
        (97.25, 103.0, 26.5, "Medium", "medium"),
        (141.45, 147.3, 19.5, "Heavy", "heavy"),
        (178.75, 184.5, 22.7, "Shields", "shield"),
    ] {
        g.diamond_tail(x, 648.65, 4.14, has(key));
        g.text(tx, 650.9, Font::Text, Anchor::Start, Some(w), t);
    }
    // (rule y, text height, label, label width, aria label, list)
    type Section<'a> = (f32, f32, &'a str, f32, &'a str, &'a Vec<String>);
    let sections: [Section; 3] = [
        (658.8, 22.0 + e3, "weapons", 35.8, "Weapon proficiencies", &s.weapon_profs),
        (692.5 + e3, 18.0 + e3, "tools", 23.8, "Tool proficiencies", &s.tool_profs),
        (722.4 + 2.0 * e3, 27.5 + e3, "languages", 44.0, "Languages", &s.languages),
    ];
    for (rule_y, h, label, label_w, aria, list) in sections {
        g.title_rule(16.0, rule_y, 191.5);
        g.text(16.0, rule_y + 7.0, Font::Label, Anchor::Start, Some(label_w), label);
        html.push_str(&text_box(16.5, rule_y + 9.7, 190.0, h, &[("", joined(list))], aria));
    }

    Page {
        svg: art::svg_page(&g.0, &format!("Character sheet page 1 for {}", s.name), art::H + ext),
        html,
    }
}

/// One column of the Actions box, filled in order until it is full, never
/// below a readable size. Up to six entries are one per line; more flow as
/// a paragraph ending with a count of what didn't fit. Spells are in italics.
fn action_column(x: f32, y: f32, w: f32, h: f32, entries: &[&rules::play::ActionEntry], label: &str) -> String {
    const MIN: f32 = 6.6;
    let name_html = |e: &rules::play::ActionEntry| if e.spell.is_some() { format!("<i>{}</i>", esc(&e.name)) } else { esc(&e.name) };
    // "+3 more (spell page)", "+1 more (Features & Traits)"
    let more_note = |rest: &[&&rules::play::ActionEntry]| {
        if rest.is_empty() {
            return String::new();
        }
        let spells = rest.iter().filter(|e| e.spell.is_some()).count();
        let place = match (spells, rest.len() - spells) {
            (_, 0) => "spell pages",
            (0, _) => "Features & Traits",
            _ => "Features & Traits, spell pages",
        };
        format!("+{} more ({place})", rest.len())
    };
    let lines_h = |lines: f32, size: f32| (lines + 0.25) * size * 1.17;
    // One per line.
    if entries.len() <= 6 {
        for size in [7.5, 7.0, MIN] {
            let lines: f32 = entries.iter().map(|e| (text_width(&e.name, size) / w).ceil().max(1.0)).sum();
            if lines_h(lines, size) <= h {
                let inner: String = entries.iter().map(|e| format!("<p>{}</p>", name_html(e))).collect();
                return html_box(x, y, w, h, size, "text", &inner, label);
            }
        }
    }
    // A paragraph: the first k entries, plus a note for what's left.
    let rest = |k: usize| entries[k..].iter().collect::<Vec<_>>();
    let para = |k: usize| {
        let mut t: Vec<String> = entries[..k].iter().map(|e| e.name.clone()).collect();
        let note = more_note(&rest(k));
        if !note.is_empty() {
            t.push(note);
        }
        t.join(", ")
    };
    let fits = |k: usize, size: f32| lines_h((text_width(&para(k), size) / w).ceil().max(1.0), size) <= h;
    let (size, k) = [7.5, 7.0, MIN]
        .into_iter()
        .find_map(|size| fits(entries.len(), size).then_some((size, entries.len())))
        .unwrap_or_else(|| (MIN, (0..=entries.len()).rev().find(|&k| fits(k, MIN)).unwrap_or(0)));
    let mut parts: Vec<String> = entries[..k].iter().map(|e| name_html(e)).collect();
    let note = more_note(&rest(k));
    if !note.is_empty() {
        parts.push(format!(r#"<span class="more">{}</span>"#, esc(&note)));
    }
    html_box(x, y, w, h, size, "text", &format!("<p>{}</p>", parts.join(", ")), label)
}


// ------------------------------------------------------------------ page 2

/// "1 action" → "Action" etc., to fit the narrow casting-time column.
fn short_time(t: &str) -> String {
    let first = t.split(',').next().unwrap_or("").trim();
    match first {
        "1 action" => "Action".into(),
        "1 bonus action" => "Bonus Action".into(),
        "1 reaction" => "Reaction".into(),
        _ => first
            .replace(" minutes", " min")
            .replace(" minute", " min")
            .replace(" hours", " hr")
            .replace(" hour", " hr"),
    }
}

fn short_duration(d: &str) -> String {
    let d = d
        .trim_start_matches("Concentration, up to ")
        .trim_start_matches("Concentration, ");
    match d {
        "Instantaneous" => "Instant".into(),
        _ => d
            .replace(" minutes", " min")
            .replace(" minute", " min")
            .replace(" hours", " hr")
            .replace(" hour", " hr"),
    }
}

/// Material components with a cost or that are consumed (the sheet's "M").
fn required_material(components: &str) -> bool {
    let Some(i) = components.find("M (") else {
        return false;
    };
    let m = components[i..].to_lowercase();
    m.contains(" gp") || m.contains("worth") || m.contains("consume")
}

fn spell_note(sp: &SpellLine, s: &Sheet) -> String {
    let mut parts = vec![short_duration(&sp.duration)];
    match sp.status.as_str() {
        "" | "Prepared" | "Known" | "Book" => {}
        "Always" => parts.push("always prepared".into()),
        other => parts.push(other.to_string()),
    }
    let from_class = s
        .classes
        .iter()
        .any(|c| sp.source_label.starts_with(&c.name));
    if !from_class && !sp.source_label.is_empty() {
        parts.push(sp.source_label.trim_start_matches("Feat: ").to_string());
    }
    parts.retain(|p| !p.is_empty());
    parts.join("; ")
}

pub fn page2(ch: &Character, s: &Sheet, ext: f32) -> Page {
    PAGE_H.with(|h| h.set(art::H + ext));
    let mut g = Svg::default();
    let mut html = String::new();
    let d = &ch.details;

    // Spellcasting ability / modifier / save DC / attack.
    g.frame(
        12.1,
        12.0,
        125.3,
        116.2,
        [Notch(8.0), Square, Chamfer(4.0), Square],
    );
    g.knob(13.6, 13.6);
    g.field(24.9, 32.9, 108.2);
    g.text(
        24.9,
        39.3,
        Font::Label,
        Anchor::Start,
        Some(81.9),
        "spellcasting ability",
    );
    g.path(
        "M19.3 44.4H137.4M12.1 72.1H137.4M12.1 100H137.4",
        BAND,
        1.1,
        "none",
    );
    g.vline(47.6, 44.4, 83.8, 1.1, BAND);
    g.path("M19.3 44.4C16 50 14.6 56 14.2 64", SOFT, 0.6, "none");
    g.text(
        56.3,
        57.3,
        Font::Head,
        Anchor::Start,
        Some(55.5),
        "SPELLCASTING",
    );
    g.text(
        56.3,
        65.3,
        Font::Head,
        Anchor::Start,
        Some(38.2),
        "MODIFIER",
    );
    g.text(
        56.3,
        89.0,
        Font::Head,
        Anchor::Start,
        Some(55.7),
        "SPELL SAVE DC",
    );
    g.text(
        56.3,
        112.7,
        Font::Head,
        Anchor::Start,
        Some(51.8),
        "SPELL ATTACK",
    );
    g.text(56.3, 120.7, Font::Head, Anchor::Start, Some(27.6), "BONUS");
    let casting = &s.casting;
    if !casting.is_empty() {
        let modv = |a: Ability| {
            s.abilities
                .iter()
                .find(|x| x.ability == a)
                .map(|x| x.modifier)
                .unwrap_or(0)
        };
        let ability = if casting.len() == 1 {
            casting[0].ability.name().to_string()
        } else {
            casting
                .iter()
                .map(|c| {
                    let label = c.label.trim_start_matches("Feat: ");
                    format!(
                        "{} ({})",
                        c.ability.abbr(),
                        label.split(';').next().unwrap_or(label).trim()
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ")
        };
        let join = |f: &dyn Fn(&rules::sheet::Casting) -> String| {
            let mut v: Vec<String> = casting.iter().map(f).collect();
            v.dedup();
            v.join("/")
        };
        g.value(25.4, 30.6, 107.0, 10.0, Anchor::Start, &ability);
        g.value(
            29.85,
            63.8,
            32.0,
            16.0,
            Anchor::Middle,
            &join(&|c| signed(modv(c.ability))),
        );
        g.value(
            29.85,
            91.6,
            32.0,
            16.0,
            Anchor::Middle,
            &join(&|c| c.save_dc.to_string()),
        );
        g.value(
            29.85,
            119.4,
            32.0,
            16.0,
            Anchor::Middle,
            &join(&|c| signed(c.attack)),
        );
    }

    // Title strip (the reference's illustrated banner): the character's name.
    g.path("M151 49.5H394M155 45.6H390", FIELD, 0.5, "none");
    g.hline(151.0, 49.5, 243.0, 1.4, BAND);
    for (x, dir) in [(152.0_f32, 1.0_f32), (393.0, -1.0)] {
        g.path(
            &format!(
                "M{x} 49.5C{} 40 {} 24 {} 12",
                x + 3.0 * dir,
                x + 9.0 * dir,
                x + 7.0 * dir
            ),
            SOFT,
            0.6,
            "none",
        );
        g.path(
            &format!(
                "M{} 49.5C{} 38 {} 26 {} 14",
                x + 8.0 * dir,
                x + 4.0 * dir,
                x + 13.0 * dir,
                x + 12.0 * dir
            ),
            SOFT,
            0.6,
            "none",
        );
    }
    g.raw(&format!(
        r#"<text x="272.4" y="31" class="title" text-anchor="middle" style="font-size:{:.2}px">{}</text>"#,
        art::fit(&s.name.to_uppercase(), 16.0, 210.0, 8.0),
        esc(&s.name.to_uppercase())
    ));
    let sub = [s.species.as_str(), s.class_line.as_str()]
        .iter()
        .filter(|x| !x.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");
    g.value(272.4, 41.5, 220.0, 7.5, Anchor::Middle, &sub);

    // Spell slots.
    g.frame(
        150.5,
        55.0,
        245.9,
        73.2,
        [Chamfer(5.0), Chamfer(5.0), Square, Square],
    );
    g.text(
        253.1,
        67.0,
        Font::Head,
        Anchor::Start,
        Some(47.8),
        "SPELL SLOTS",
    );
    g.title_rule(154.5, 73.2, 237.8);
    g.vline(239.2, 80.3, 43.1, 0.5, SOFT);
    g.vline(317.9, 80.3, 43.1, 0.5, SOFT);
    let printed = [4, 3, 3, 3, 3, 2, 2, 1, 1];
    for (col, (lx, tx, dx)) in [
        (154.8, 181.8, 198.7),
        (242.8, 269.9, 286.7),
        (321.4, 348.4, 365.4),
    ]
    .into_iter()
    .enumerate()
    {
        g.text(tx, 82.5, Font::Column, Anchor::Start, Some(13.7), "Total");
        g.text(
            tx + 17.5,
            82.5,
            Font::Column,
            Anchor::Start,
            Some(27.8),
            "Expended",
        );
        g.hline(tx, 83.6, 14.4, 0.25, FIELD);
        g.hline(tx + 17.5, 83.6, 27.6, 0.25, FIELD);
        for row in 0..3 {
            let lvl = col * 3 + row + 1;
            let (dy, ly) = ([86.3, 100.4, 114.4][row], [94.3, 108.4, 122.4][row]);
            g.text(
                lx,
                ly - 1.1,
                Font::LabelSmall,
                Anchor::Start,
                Some(22.0),
                &format!("Level {lvl}"),
            );
            g.field(tx, ly, 14.4);
            let slots = s.slots[lvl - 1] as usize;
            let pact = s
                .pact_slots
                .filter(|(_, l)| *l as usize == lvl)
                .map(|(n, _)| n as usize)
                .unwrap_or(0);
            let total = match (slots, pact) {
                (0, 0) => String::new(),
                (n, 0) => n.to_string(),
                (0, p) => format!("{p}P"),
                (n, p) => format!("{n}+{p}P"),
            };
            g.value(tx + 7.2, ly - 1.2, 15.0, 7.5, Anchor::Middle, &total);
            let n = printed[lvl - 1].max((slots + pact).min(4));
            for i in 0..n {
                g.diamond(dx + 4.15 + i as f32 * 9.5, dy + 4.15, 4.14, 1.0, false);
            }
        }
    }

    // Cantrips & prepared spells.
    g.frame(
        12.1,
        138.8,
        384.0,
        625.2 + ext,
        [Square, Chamfer(6.0), Square, Square],
    );
    // Longer paper: the table gets as many extra rows as fit.
    let rows = 30 + (ext / 19.44).floor() as usize;
    g.text(
        138.6,
        151.0,
        Font::Head,
        Anchor::Start,
        Some(114.8),
        "CANTRIPS & PREPARED SPELLS",
    );
    g.title_rule(16.2, 157.2, 375.9);
    for x in [40.1, 152.4, 186.5, 232.1, 302.5] {
        g.vline(x, 163.0, 594.0 + ext, 0.5, DIVIDER);
    }
    g.text(
        19.1,
        174.6,
        Font::Column,
        Anchor::Start,
        Some(14.6),
        "Level",
    );
    g.text(42.4, 174.6, Font::Column, Anchor::Start, Some(17.6), "Name");
    g.text(
        154.7,
        168.2,
        Font::Column,
        Anchor::Start,
        Some(21.5),
        "Casting",
    );
    g.text(
        154.7,
        174.6,
        Font::Column,
        Anchor::Start,
        Some(14.5),
        "Time",
    );
    g.text(
        188.8,
        174.6,
        Font::Column,
        Anchor::Start,
        Some(17.8),
        "Range",
    );
    g.text(
        234.5,
        168.3,
        Font::Column,
        Anchor::Start,
        Some(63.4),
        "Concentration, Ritual",
    );
    g.text(
        234.5,
        174.7,
        Font::Column,
        Anchor::Start,
        Some(56.7),
        "& Required Material",
    );
    g.text(
        305.0,
        174.6,
        Font::Column,
        Anchor::Start,
        Some(17.3),
        "Notes",
    );
    for (x, w) in [
        (19.1, 14.8),
        (42.4, 18.1),
        (154.7, 21.7),
        (188.8, 17.8),
        (234.5, 61.0),
        (305.0, 17.5),
    ] {
        g.hline(x, 175.7, w, 0.25, FIELD);
    }
    // Cantrips and prepared/always-available spells fill the table, then
    // spellbook entries that aren't prepared. A long list packs the rows
    // closer (down to two thirds of their height, text never below ~6pt);
    // past that, the last row counts what's left.
    let (mut list, book): (Vec<&SpellLine>, Vec<&SpellLine>) =
        s.spells.iter().partition(|x| x.status != "Book");
    list.extend(book);
    let area = rows as f32 * 19.44;
    let pitch = if list.len() > rows { (area / list.len() as f32).max(13.0) } else { 19.44 };
    let fit_rows = (area / pitch + 0.01).floor() as usize;
    let k = pitch / 19.44;
    let shown = if list.len() > fit_rows { fit_rows - 1 } else { list.len() };
    let small = |size: f32, min: f32| (size * k).max(min).min(size);
    for i in 0..fit_rows {
        let top = 181.0 + pitch * i as f32;
        let y = top + 11.6 * k;
        g.field(17.6, y, 20.3);
        g.field(42.4, y, 107.8);
        g.field(154.7, y, 29.4);
        g.field(188.7, y, 41.2);
        g.field(305.0, y, 85.8);
        let sp = if i < shown { list.get(i) } else { None };
        let marks = sp
            .map(|x| (x.concentration, x.ritual, required_material(&x.components)))
            .unwrap_or_default();
        for (j, (x, letter, on)) in [
            (239.0, "C", marks.0),
            (260.9, "R", marks.1),
            (282.6, "M", marks.2),
        ]
        .into_iter()
        .enumerate()
        {
            g.diamond_tail(x + 4.15, top + 4.15 * k, 4.14 * k, on);
            g.text([249.1, 270.8, 292.5][j], top + 6.4 * k, Font::Faint, Anchor::Start, None, letter);
        }
        let bl = y - 3.0 * k.max(0.8);
        if let Some(sp) = sp {
            g.value_min(27.75, bl, 20.0, small(7.5, 6.2), 6.2, Anchor::Middle, &if sp.level == 0 { "Cantrip".to_string() } else { sp.level.to_string() });
            g.value_min(42.8, bl, 107.0, small(7.8, 6.6), 6.6, Anchor::Start, &sp.name);
            g.value_min(155.0, bl, 29.0, small(6.5, 6.0), 6.0, Anchor::Start, &short_time(&sp.time));
            g.value_min(189.0, bl, 40.8, small(6.5, 6.0), 6.0, Anchor::Start, &sp.range);
            g.value_min(305.4, bl, 85.0, small(6.2, 6.0), 6.0, Anchor::Start, &spell_note(sp, s));
        } else if i == shown && list.len() > shown {
            let more = list.len() - shown;
            g.note(42.8, bl, 260.0, 6.6, 6.0, &format!("+{more} more spells — see the Spell Descriptions pages"));
        }
    }

    // Appearance.
    let right_box = |g: &mut Svg, y: f32, h: f32, c: [Corner; 4]| g.frame(407.3, y, 183.7, h, c);
    // Its bottom lines up with the spellcasting and spell slot boxes.
    right_box(
        &mut g,
        12.2,
        116.0,
        [Square, Notch(8.0), Square, Chamfer(4.0)],
    );
    g.knob(589.4, 13.6);
    g.text(
        474.4,
        24.3,
        Font::Head,
        Anchor::Start,
        Some(49.2),
        "APPEARANCE",
    );
    g.grid(407.3, 30.5, 183.7, 97.7);
    g.title_rule(411.4, 30.5, 175.5);
    let looks: Vec<String> = [
        ("Age", &d.age),
        ("Height", &d.height),
        ("Weight", &d.weight),
        ("Eyes", &d.eyes),
        ("Hair", &d.hair),
        ("Skin", &d.skin),
        ("Pronouns", &d.pronouns),
        ("Faith", &d.faith),
    ]
    .iter()
    .filter(|(_, v)| !v.trim().is_empty())
    .map(|(l, v)| format!("{l}: {}", v.trim()))
    .collect();
    html.push_str(&text_box(
        412.6,
        33.0,
        173.5,
        93.5,
        &[("", looks.join(" · ")), ("", d.appearance.clone())],
        "Appearance",
    ));

    // Backstory & personality, alignment: level with the spell table.
    let (by, bh) = (138.8, 212.6);
    right_box(&mut g, by, bh, [Chamfer(4.0); 4]);
    g.text(
        446.0,
        by + 12.3,
        Font::Head,
        Anchor::Start,
        Some(106.0),
        "BACKSTORY & PERSONALITY",
    );
    let text_h = bh - 18.4 - 33.2;
    g.grid(407.3, by + 18.4, 183.7, text_h);
    g.title_rule(411.4, by + 18.4, 175.5);
    g.text(
        412.7,
        by + bh - 24.5,
        Font::Text,
        Anchor::Start,
        Some(31.7),
        "Alignment",
    );
    g.field(413.6, by + bh - 7.6, 171.8);
    g.value(414.0, by + bh - 9.9, 171.0, 9.0, Anchor::Start, &d.alignment);
    let story = [
        ("Personality:", d.personality.clone()),
        ("Ideals:", d.ideals.clone()),
        ("Bonds:", d.bonds.clone()),
        ("Flaws:", d.flaws.clone()),
        ("Backstory:", d.backstory.clone()),
        ("Allies & organizations:", d.allies.clone()),
        ("Notes:", d.notes.clone()),
    ];
    html.push_str(&text_box(
        412.6,
        by + 21.0,
        173.5,
        text_h - 4.0,
        &story,
        "Backstory and personality",
    ));

    // (Languages are on page 1, with the other proficiencies.)

    // Equipment and attunement.
    // Equipment fills the column down to the coins, which sit at the very bottom.
    right_box(&mut g, 362.0, 316.3 + ext, [Chamfer(4.0); 4]);
    g.text(
        475.9,
        374.3,
        Font::Head,
        Anchor::Start,
        Some(46.2),
        "EQUIPMENT",
    );
    g.grid(407.3, 380.4, 183.7, 225.0 + ext);
    g.title_rule(411.4, 380.4, 175.5);
    // Attunement lines sit at the bottom of the box: move them down with it.
    g.raw(&format!(r#"<g transform="translate(0 {})">"#, ext + 20.0));
    g.text(
        412.7,
        593.8,
        Font::Text,
        Anchor::Start,
        Some(72.8),
        "Magic Item Attunement",
    );
    let attuned: Vec<String> = s
        .inventory
        .iter()
        .filter(|i| i.attuned)
        .map(|i| i.name.clone())
        .collect();
    for (i, y) in [610.7, 630.7, 650.7].into_iter().enumerate() {
        g.field(413.6, y, 171.8);
        g.diamond_tail(
            423.25,
            603.25 + 20.0 * i as f32,
            4.14,
            attuned.get(i).is_some(),
        );
        if let Some(n) = attuned.get(i) {
            // More than three (a house rule): the last line counts the rest.
            let n = if i == 2 && attuned.len() > 3 { format!("{n} (+{} more)", attuned.len() - 3) } else { n.clone() };
            g.value(431.0, y - 2.2, 154.0, 8.5, Anchor::Start, &n);
        }
    }
    g.raw("</g>");
    let items: Vec<Item> = s
        .inventory
        .iter()
        .map(|i| Item {
            name: if i.qty > 1 {
                format!("{} ×{}", i.name, i.qty)
            } else {
                i.name.clone()
            },
            // No equipped/attuned marks: what's equipped changes during play,
            // and attunement has its own lines below the list.
            tag: String::new(),
            detail: i.notes.clone(),
        })
        .collect();
    let h = list_box(412.6, 383.0, 173.5, 212.0 + ext, 2, 8.0, items, "Equipment", "items not shown");
    html.push_str(&h.replacen("rp-box list", "rp-box list equip", 1));
    g.value(
        585.4,
        603.2 + ext,
        173.0,
        6.0,
        Anchor::End,
        &format!(
            "Carried {:.1} lb. · capacity {} lb.",
            s.carried_weight, s.carry_capacity
        ),
    );

    // Coins: same size, flush with the bottom of the spell table (764.0).
    g.raw(&format!(r#"<g transform="translate(0 {})">"#, ext + 19.3));
    right_box(
        &mut g,
        669.5,
        75.2,
        [Chamfer(4.0), Square, Notch(8.0), Square],
    );
    g.knob(588.6, 742.4);
    g.text(486.8, 680.6, Font::Head, Anchor::Start, Some(24.5), "COINS");
    g.title_rule(411.4, 686.7, 175.5);
    let c = &s.coins;
    for (i, (label, lx, v)) in [
        ("CP", 424.3, c.cp),
        ("SP", 460.4, c.sp),
        ("EP", 495.8, c.ep),
        ("GP", 530.4, c.gp),
        ("PP", 566.0, c.pp),
    ]
    .into_iter()
    .enumerate()
    {
        let bx = [413.1, 448.9, 484.4, 519.5, 554.6][i];
        let sx = bx + 2.34;
        g.text(lx, 697.4, Font::Faint, Anchor::Start, None, label);
        let star = format!(
            "M{} 713.82C{} 717.09 {} 719.75 {} 727.03C{} 719.75 {} 717.09 {} 713.82C{} 710.54 {} 707.88 {} 700.6C{} 707.88 {} 710.54 {} 713.82Z",
            sx + 25.92,
            sx + 18.78,
            sx + 16.18,
            sx + 12.96,
            sx + 9.74,
            sx + 7.14,
            sx,
            sx + 7.14,
            sx + 9.74,
            sx + 12.96,
            sx + 16.18,
            sx + 18.78,
            sx + 25.92
        );
        g.path(&star, INK, 0.75, "none");
        let b = format!(
            "M{bx} 703.44V721.19C{bx} 721.19 {} 721.19 {} 723.71H{}C{} 723.71 {} 721.19 {} 721.19V703.44Z",
            bx + 2.52,
            bx + 2.52,
            bx + 28.02,
            bx + 28.02,
            bx + 28.02,
            bx + 30.54
        );
        g.path(&b, FIELD, 1.0, "#fff");
        g.hline(bx, 704.3, 30.5, 0.75, INK);
        if v > 0 {
            g.value(
                bx + 15.27,
                717.6,
                28.0,
                10.0,
                Anchor::Middle,
                &v.to_string(),
            );
        }
    }
    g.raw("</g>");

    Page {
        svg: art::svg_page(&g.0, &format!("Character sheet page 2 for {}", s.name), art::H + ext),
        html,
    }
}
