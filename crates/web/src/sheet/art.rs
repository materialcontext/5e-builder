//! A tiny SVG writer for the printed sheet.
//!
//! Everything is drawn in the reference sheet's own coordinate system
//! (603 × 774 units, one unit = one PDF point of the original), so each
//! measurement can be copied straight from the reference.

// Drawing helpers take coordinates as plain numbers.
#![allow(clippy::too_many_arguments)]

use std::fmt::Write;

pub const W: f32 = 603.0;
pub const H: f32 = 774.0;

/// Ink colours measured from the reference sheet.
pub const INK: &str = "#231f20";
pub const FIELD: &str = "#b2b1b1";
pub const DIVIDER: &str = "#d3d2d2";
pub const SOFT: &str = "#9c9a9b";
pub const BAND: &str = "#a3a1a1";
pub const GRID: &str = "#e8e8e8";

/// Text styles (CSS classes in style.css, `.rp` section).
#[derive(Clone, Copy)]
pub enum Font {
    /// Bold capitals for box titles (8 pt).
    Head,
    /// Small-caps field label (9 pt).
    Label,
    /// Small-caps, smaller (7.5 pt) — "modifier", "score", "Level 1".
    LabelSmall,
    /// Regular text (7.5 pt) — skills, armor types.
    Text,
    /// Bold text (7.5 pt) — "Saving Throw".
    TextBold,
    /// Grey column header (7 pt).
    Column,
    /// Light grey marker letters/coin names (7–7.5 pt).
    Faint,
}

impl Font {
    fn class(self) -> &'static str {
        match self {
            Font::Head => "h",
            Font::Label => "sc",
            Font::LabelSmall => "sc sm",
            Font::Text => "t",
            Font::TextBold => "t b",
            Font::Column => "col",
            Font::Faint => "faint",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// How a frame corner is finished.
#[derive(Clone, Copy, PartialEq)]
pub enum Corner {
    Square,
    /// Concave quarter circle cut into the corner.
    Notch(f32),
    /// Straight 45° cut.
    Chamfer(f32),
    /// Convex rounding.
    Round(f32),
}

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// Rough advance width of `s` in Alegreya Sans at `size`, used to shrink
/// values that would overflow their field.
pub fn text_width(s: &str, size: f32) -> f32 {
    s.chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' | ' ' => 0.26,
            'f' | 't' | 'r' | 'I' | '(' | ')' | '/' | '-' => 0.34,
            'm' | 'w' | 'M' | 'W' => 0.78,
            'A'..='Z' => 0.6,
            '0'..='9' | '+' => 0.5,
            _ => 0.47,
        })
        .sum::<f32>()
        * size
}

/// Largest size ≤ `size` (down to `min`) at which `s` fits in `w`.
/// `s` cut at a word (or character) boundary with "…" so it fits `w` at `size`.
pub fn clip(s: &str, size: f32, w: f32) -> String {
    if text_width(s, size) <= w {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        if text_width(&format!("{out}{ch}…"), size) > w {
            break;
        }
        out.push(ch);
    }
    // Prefer ending at a word boundary when one is reasonably close.
    if let Some(i) = out.rfind([' ', ',']) {
        if i * 4 > out.len() * 3 {
            out.truncate(i);
        }
    }
    format!("{}…", out.trim_end_matches([' ', ',']))
}

pub fn fit(s: &str, size: f32, w: f32, min: f32) -> f32 {
    let natural = text_width(s, size);
    if natural <= w {
        size
    } else {
        (size * w / natural).max(min)
    }
}

#[derive(Default)]
pub struct Svg(pub String);

impl Svg {
    pub fn raw(&mut self, s: &str) {
        self.0.push_str(s);
    }

    /// Horizontal rule from `x` of width `w`.
    pub fn hline(&mut self, x: f32, y: f32, w: f32, sw: f32, color: &str) {
        let _ = write!(
            self.0,
            r#"<path d="M{x} {y}h{w}" stroke="{color}" stroke-width="{sw}"/>"#
        );
    }

    pub fn vline(&mut self, x: f32, y: f32, h: f32, sw: f32, color: &str) {
        let _ = write!(
            self.0,
            r#"<path d="M{x} {y}v{h}" stroke="{color}" stroke-width="{sw}"/>"#
        );
    }

    /// A field line a value is written on.
    pub fn field(&mut self, x: f32, y: f32, w: f32) {
        self.hline(x, y, w, 0.5, FIELD);
    }

    /// Title rule under a box heading.
    pub fn title_rule(&mut self, x: f32, y: f32, w: f32) {
        self.hline(x, y, w, 0.75, INK);
    }

    pub fn path(&mut self, d: &str, stroke: &str, sw: f32, fill: &str) {
        let _ = write!(
            self.0,
            r#"<path d="{d}" stroke="{stroke}" stroke-width="{sw}" fill="{fill}"/>"#
        );
    }

    /// Text at baseline (`x`, `y`). `len` stretches the letter spacing to the
    /// reference width so labels line up exactly.
    pub fn text(&mut self, x: f32, y: f32, font: Font, anchor: Anchor, len: Option<f32>, s: &str) {
        let a = match anchor {
            Anchor::Start => "",
            Anchor::Middle => r#" text-anchor="middle""#,
            Anchor::End => r#" text-anchor="end""#,
        };
        let l = len
            .map(|l| format!(r#" textLength="{l}" lengthAdjust="spacing""#))
            .unwrap_or_default();
        let _ = write!(
            self.0,
            r#"<text x="{x}" y="{y}" class="{}"{a}{l}>{}</text>"#,
            font.class(),
            esc(s)
        );
    }

    /// A filled-in value, shrunk to fit `w` (never below `min`).
    pub fn value(&mut self, x: f32, y: f32, w: f32, size: f32, anchor: Anchor, s: &str) {
        if s.is_empty() {
            return;
        }
        let sz = fit(s, size, w, size.min(5.0));
        let a = match anchor {
            Anchor::Start => "",
            Anchor::Middle => r#" text-anchor="middle""#,
            Anchor::End => r#" text-anchor="end""#,
        };
        // Below the minimum size, squeeze horizontally as a last resort.
        let squeeze = if text_width(s, sz) > w * 1.02 {
            format!(r#" textLength="{w}" lengthAdjust="spacingAndGlyphs""#)
        } else {
            String::new()
        };
        let _ = write!(
            self.0,
            r#"<text x="{x}" y="{y}" class="v"{a}{squeeze} style="font-size:{sz:.2}px">{}</text>"#,
            esc(s)
        );
    }

    /// A secondary line under a value (grey italic): properties, notes.
    /// Never smaller than `min`: what doesn't fit is cut with "…".
    pub fn note(&mut self, x: f32, y: f32, w: f32, size: f32, min: f32, s: &str) {
        if s.is_empty() {
            return;
        }
        let sz = fit(s, size, w, min);
        let t = clip(s, sz, w);
        let _ = write!(self.0, r#"<text x="{x}" y="{y}" class="note" style="font-size:{sz:.2}px">{}</text>"#, esc(&t));
    }

    /// Like [`Svg::value`], but never smaller than `min`: what doesn't fit is
    /// cut with "…" instead of shrunk or squeezed.
    #[allow(clippy::too_many_arguments)]
    pub fn value_min(&mut self, x: f32, y: f32, w: f32, size: f32, min: f32, anchor: Anchor, s: &str) {
        if s.is_empty() {
            return;
        }
        let sz = fit(s, size, w, min);
        let t = clip(s, sz, w);
        let a = match anchor {
            Anchor::Start => "",
            Anchor::Middle => r#" text-anchor="middle""#,
            Anchor::End => r#" text-anchor="end""#,
        };
        let _ = write!(self.0, r#"<text x="{x}" y="{y}" class="v"{a} style="font-size:{sz:.2}px">{}</text>"#, esc(&t));
    }

    /// The sheet's four-pointed concave "diamond" marker, centred on (cx, cy)
    /// with half-size `a`. Filled black when `on`.
    pub fn diamond(&mut self, cx: f32, cy: f32, a: f32, sw: f32, on: bool) {
        let k = a * 0.755;
        let d = format!(
            "M{cx} {b}C{cx} {c1} {l1} {cy} {l} {cy}C{l1} {cy} {cx} {t1} {cx} {t}C{cx} {t1} {r1} {cy} {r} {cy}C{r1} {cy} {cx} {c1} {cx} {b}Z",
            b = cy + a,
            c1 = cy + k,
            l1 = cx - k,
            l = cx - a,
            t1 = cy - k,
            t = cy - a,
            r1 = cx + k,
            r = cx + a,
        );
        self.path(&d, INK, sw, if on { INK } else { "#fff" });
    }

    /// Diamond with the soft lead-in line the reference draws to its left.
    pub fn diamond_tail(&mut self, cx: f32, cy: f32, a: f32, on: bool) {
        let _ = write!(
            self.0,
            r#"<path d="M{} {cy}h{}" stroke="url(#tail)" stroke-width="0.9"/>"#,
            cx - a - 6.5,
            6.8
        );
        self.diamond(cx, cy, a, 1.0, on);
    }

    /// Proficiency bubble. `fill`: 0 empty, 1 half, 2 full, 3 full + ring (expertise).
    pub fn bubble(&mut self, cx: f32, cy: f32, fill: u8) {
        let r = 3.1;
        match fill {
            0 => self.circle(cx, cy, r, INK, 1.0, "#fff"),
            1 => {
                self.circle(cx, cy, r, INK, 1.0, "#fff");
                let _ = write!(
                    self.0,
                    r#"<path d="M{cx} {}A{r} {r} 0 0 0 {cx} {}Z" fill="{INK}"/>"#,
                    cy - r,
                    cy + r
                );
            }
            _ => {
                self.circle(cx, cy, r, INK, 1.0, INK);
                if fill >= 3 {
                    self.circle(cx, cy, r + 1.6, INK, 0.6, "none");
                }
            }
        }
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, stroke: &str, sw: f32, fill: &str) {
        let _ = write!(
            self.0,
            r#"<circle cx="{cx}" cy="{cy}" r="{r}" stroke="{stroke}" stroke-width="{sw}" fill="{fill}"/>"#
        );
    }

    pub fn ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        stroke: &str,
        sw: f32,
        fill: &str,
    ) {
        let _ = write!(
            self.0,
            r#"<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" stroke="{stroke}" stroke-width="{sw}" fill="{fill}"/>"#
        );
    }

    /// The 5.4-unit note grid, clipped to a rectangle (aligned to the page,
    /// as in the reference, so grids in neighbouring boxes line up).
    pub fn grid(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let mut d = String::new();
        let step = 5.4;
        let mut gx = 0.9 + ((x - 0.9) / step).ceil() * step;
        while gx < x + w {
            let _ = write!(d, "M{gx:.2} {y}v{h}");
            gx += step;
        }
        let mut gy = (y / step).ceil() * step;
        while gy < y + h {
            let _ = write!(d, "M{x} {gy:.2}h{w}");
            gy += step;
        }
        let _ = write!(
            self.0,
            r#"<path d="{d}" stroke="{GRID}" stroke-width="0.5"/>"#
        );
    }

    /// A rectangle outline with individually finished corners (tl, tr, br, bl).
    pub fn rect_path(x: f32, y: f32, w: f32, h: f32, c: [Corner; 4]) -> String {
        let (x2, y2) = (x + w, y + h);
        let mut d = String::new();
        // Start on the left edge just below the top-left corner, go clockwise.
        let inset = |c: Corner| match c {
            Corner::Square => 0.0,
            Corner::Notch(r) | Corner::Chamfer(r) | Corner::Round(r) => r,
        };
        let _ = write!(d, "M{x} {}", y + inset(c[0]));
        corner(&mut d, c[0], x + inset(c[0]), y, 0);
        let _ = write!(d, "L{} {y}", x2 - inset(c[1]));
        corner(&mut d, c[1], x2, y + inset(c[1]), 1);
        let _ = write!(d, "L{x2} {}", y2 - inset(c[2]));
        corner(&mut d, c[2], x2 - inset(c[2]), y2, 2);
        let _ = write!(d, "L{} {y2}", x + inset(c[3]));
        corner(&mut d, c[3], x, y2 - inset(c[3]), 3);
        d.push('Z');
        d
    }

    /// The printed frame: a wide grey band with a fine inner line, white inside.
    /// (`x`, `y`, `w`, `h`) is the white interior, as measured on the reference.
    pub fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, c: [Corner; 4]) {
        let b = 2.7;
        let grow = |c: Corner| match c {
            Corner::Square => Corner::Square,
            Corner::Notch(r) => Corner::Notch(r + b / 2.0),
            Corner::Chamfer(r) => Corner::Chamfer(r + b / 2.0 * 0.41),
            Corner::Round(r) => Corner::Round(r + b / 2.0),
        };
        let outer = Self::rect_path(x - b / 2.0, y - b / 2.0, w + b, h + b, c.map(grow));
        self.path(&outer, BAND, b, "#fff");
        let shrink = |c: Corner| match c {
            Corner::Square => Corner::Square,
            Corner::Notch(r) => Corner::Notch((r - 1.6).max(0.5)),
            Corner::Chamfer(r) => Corner::Chamfer((r - 0.7).max(0.5)),
            Corner::Round(r) => Corner::Round((r - 1.6).max(0.5)),
        };
        let inner = Self::rect_path(x + 1.6, y + 1.6, w - 3.2, h - 3.2, c.map(shrink));
        self.path(&inner, "#c9c7c7", 0.5, "none");
    }

    /// Grey knob ornament sitting on a frame corner.
    pub fn knob(&mut self, cx: f32, cy: f32) {
        self.circle(cx, cy, 3.3, "#8e8c8c", 0.6, BAND);
    }
}

/// Appends the corner segment ending at (ex, ey). `i` is the corner index
/// (0 tl, 1 tr, 2 br, 3 bl), which fixes the arc centre.
fn corner(d: &mut String, c: Corner, ex: f32, ey: f32, _i: usize) {
    match c {
        Corner::Square => {}
        Corner::Chamfer(_) => {
            let _ = write!(d, "L{ex} {ey}");
        }
        Corner::Notch(r) => {
            let _ = write!(d, "A{r} {r} 0 0 0 {ex} {ey}");
        }
        Corner::Round(r) => {
            let _ = write!(d, "A{r} {r} 0 0 1 {ex} {ey}");
        }
    }
}

/// Wraps drawing into a full-page SVG with the shared defs.
pub fn svg_page(body: &str, label: &str, height: f32) -> String {
    format!(
        r##"<svg class="rp-art" viewBox="0 0 {W} {height}" xmlns="http://www.w3.org/2000/svg" aria-label="{}"><defs><linearGradient id="tail" x1="0" x2="1" y1="0" y2="0"><stop offset="0" stop-color="{INK}" stop-opacity="0"/><stop offset="1" stop-color="{INK}" stop-opacity=".85"/></linearGradient><linearGradient id="fade" x1="0" x2="1" y1="0" y2="0"><stop offset="0" stop-color="{FIELD}"/><stop offset=".7" stop-color="{FIELD}"/><stop offset="1" stop-color="{FIELD}" stop-opacity="0"/></linearGradient></defs><g fill="none">{body}</g></svg>"##,
        esc(label)
    )
}
