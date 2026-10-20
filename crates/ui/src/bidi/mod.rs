//! Right-to-left layout and bidirectional text for egui.
//!
//! egui lays rows out left to right and draws a string's characters in memory order. For an
//! Arabic or Hebrew interface the app picks a [`Direction`] once (from `craft_i18n::I18n::direction`)
//! and uses it for every horizontal arrangement: [`horizontal`] rows, [`Direction::mirror_rect`]
//! for hand-placed rectangles, [`crate::layout::SplitLayout::mirrored`] for splits, and
//! [`Direction::logical_dx`] / [`Direction::logical_key`] so dragging and arrow keys follow the
//! mirrored order. [`display`] puts a label's text into visual order with the Unicode
//! Bidirectional Algorithm (the `unicode-bidi` crate), so mixed Hebrew, Arabic, Latin and digits
//! read correctly when egui draws it.
//!
//! [`display`] works on single lines: wrap first, then reorder each line. It does not shape
//! Arabic (joining letters into their contextual forms); that needs a font renderer with shaping,
//! which egui does not have, so Arabic letters are drawn in their isolated forms.

use std::borrow::Cow;

use egui::{Align, InnerResponse, Key, Layout, Rect, Ui};
use unicode_bidi::{Level, ParagraphBidiInfo};

#[cfg(test)]
mod tests;

/// The direction an interface or a paragraph runs in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
    /// Left to right.
    #[default]
    Ltr,
    /// Right to left: Arabic, Hebrew, Persian, Urdu and the like.
    Rtl,
}

impl Direction {
    /// `Rtl` when `rtl`: `Direction::from_rtl(i18n.rtl(lang))`.
    pub fn from_rtl(rtl: bool) -> Self {
        if rtl { Self::Rtl } else { Self::Ltr }
    }

    /// Right to left.
    pub fn is_rtl(self) -> bool {
        self == Self::Rtl
    }

    /// The direction of `text`'s first strong letter (rules P2 and P3 of the bidi algorithm),
    /// or `None` when it has none (digits, punctuation, an empty string).
    pub fn of_text(text: &str) -> Option<Self> {
        match unicode_bidi::get_base_direction(text) {
            unicode_bidi::Direction::Ltr => Some(Self::Ltr),
            unicode_bidi::Direction::Rtl => Some(Self::Rtl),
            unicode_bidi::Direction::Mixed => None,
        }
    }

    /// A row layout that starts at the leading edge: `left_to_right`, or `right_to_left` so the
    /// first widget added sits at the right.
    pub fn row(self, valign: Align) -> Layout {
        match self {
            Self::Ltr => Layout::left_to_right(valign),
            Self::Rtl => Layout::right_to_left(valign),
        }
    }

    /// The horizontal alignment of the leading edge: `Min` (left) or `Max` (right). Use it for
    /// text and for a column's cross alignment.
    pub fn leading(self) -> Align {
        match self {
            Self::Ltr => Align::Min,
            Self::Rtl => Align::Max,
        }
    }

    /// The horizontal alignment of the trailing edge, where a row's secondary controls go.
    pub fn trailing(self) -> Align {
        match self {
            Self::Ltr => Align::Max,
            Self::Rtl => Align::Min,
        }
    }

    /// `x` mirrored inside `within` for `Rtl`; unchanged for `Ltr`.
    pub fn mirror_x(self, x: f32, within: Rect) -> f32 {
        match self {
            Self::Ltr => x,
            Self::Rtl => within.min.x + within.max.x - x,
        }
    }

    /// `rect` mirrored left to right inside `within` for `Rtl`, keeping its size and vertical
    /// position; unchanged for `Ltr`. Lay out as for left to right, then mirror.
    pub fn mirror_rect(self, rect: Rect, within: Rect) -> Rect {
        match self {
            Self::Ltr => rect,
            Self::Rtl => Rect::from_x_y_ranges(self.mirror_x(rect.max.x, within)..=self.mirror_x(rect.min.x, within), rect.y_range()),
        }
    }

    /// A horizontal movement on screen as a movement towards the trailing edge: negated for
    /// `Rtl`. For a divider drag on a mirrored split, a slider, or a horizontal scrubber.
    pub fn logical_dx(self, dx: f32) -> f32 {
        match self {
            Self::Ltr => dx,
            Self::Rtl => -dx,
        }
    }

    /// An arrow key as the key it means in left-to-right order: Left and Right swap for `Rtl`,
    /// so "Right" still means "next" in code written for left to right. Other keys pass through.
    pub fn logical_key(self, key: Key) -> Key {
        match (self, key) {
            (Self::Rtl, Key::ArrowLeft) => Key::ArrowRight,
            (Self::Rtl, Key::ArrowRight) => Key::ArrowLeft,
            _ => key,
        }
    }
}

/// A horizontal row that runs in `direction`: the first widget `add` places is at the leading
/// edge. Rows nested inside follow `ui`'s layout, so call this for each mirrored row.
pub fn horizontal<R>(ui: &mut Ui, direction: Direction, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    ui.with_layout(direction.row(Align::Center), add)
}

/// `text` in visual order, for egui to draw left to right: each line (split at `\n`) reordered
/// by the Unicode Bidirectional Algorithm, with right-to-left runs reversed (combining marks kept
/// after their letter) and their brackets mirrored, and the invisible direction marks and
/// isolates (LRM, RLM, ALM, LRE…PDF, LRI…PDI) removed once applied. `base` is the paragraph
/// direction, normally the interface's; `None` takes each line's first strong letter.
/// Text with nothing to reorder is returned as it is.
pub fn display(text: &str, base: Option<Direction>) -> Cow<'_, str> {
    let rtl_base = base == Some(Direction::Rtl);
    if !text.chars().any(|c| is_rtl_char(c) || is_bidi_control(c) || (rtl_base && !c.is_ascii_alphanumeric() && c != ' ')) {
        return Cow::Borrowed(text);
    }
    let level = base.map(|d| if d.is_rtl() { Level::rtl() } else { Level::ltr() });
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        display_line(line, level, &mut out);
    }
    if out == text { Cow::Borrowed(text) } else { Cow::Owned(out) }
}

fn display_line(line: &str, level: Option<Level>, out: &mut String) {
    if line.is_empty() {
        return;
    }
    let info = ParagraphBidiInfo::new(line, level);
    let (levels, runs) = info.visual_runs(0..line.len());
    for run in runs {
        let Some(text) = line.get(run.clone()) else {
            continue;
        };
        if levels.get(run.start).is_some_and(Level::is_rtl) {
            for cluster in clusters(text).into_iter().rev() {
                out.extend(cluster.chars().filter(|&c| !is_bidi_control(c)).map(mirrored));
            }
        } else {
            out.extend(text.chars().filter(|&c| !is_bidi_control(c)));
        }
    }
}

/// `text` split before every character that is not a combining mark, so a reversed run keeps
/// each mark after its letter.
fn clusters(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if i > start && !is_combining(c) {
            out.extend(text.get(start..i));
            start = i;
        }
    }
    out.extend(text.get(start..));
    out
}

/// Nonspacing marks of the scripts a Craft app shows: general Latin diacritics, Hebrew points
/// and cantillation, Arabic harakat, joiners and variation selectors.
fn is_combining(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x05BF | 0x05C1..=0x05C2 | 0x05C4..=0x05C5 | 0x05C7
        | 0x0610..=0x061A | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06DC | 0x06DF..=0x06E4 | 0x06E7..=0x06E8 | 0x06EA..=0x06ED
        | 0x0711 | 0x0730..=0x074A | 0x07A6..=0x07B0 | 0x07EB..=0x07F3 | 0x08D3..=0x08FF
        | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x200C..=0x200D | 0x20D0..=0x20FF | 0xFB1E | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F)
}

/// Letters of the right-to-left scripts and their presentation forms. A quick test for whether
/// the algorithm has anything to do; the algorithm itself uses the full Unicode data.
fn is_rtl_char(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// The invisible characters that only steer the algorithm: LRM, RLM, ALM and the embeddings,
/// overrides and isolates.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// The mirror image of a bracket or other paired sign (Unicode's Bidi_Mirroring_Glyph for the
/// characters a label uses), for a character drawn in a right-to-left run.
fn mirrored(c: char) -> char {
    const PAIRS: &[(char, char)] = &[
        ('(', ')'),
        ('[', ']'),
        ('{', '}'),
        ('<', '>'),
        ('«', '»'),
        ('‹', '›'),
        ('⁅', '⁆'),
        ('⁽', '⁾'),
        ('₍', '₎'),
        ('≤', '≥'),
        ('≪', '≫'),
        ('⟨', '⟩'),
        ('⟦', '⟧'),
        ('〈', '〉'),
        ('《', '》'),
        ('「', '」'),
        ('『', '』'),
        ('【', '】'),
        ('〔', '〕'),
        ('（', '）'),
        ('［', '］'),
        ('｛', '｝'),
        ('＜', '＞'),
    ];
    PAIRS
        .iter()
        .find_map(|&(a, b)| {
            if c == a {
                Some(b)
            } else if c == b {
                Some(a)
            } else {
                None
            }
        })
        .unwrap_or(c)
}
