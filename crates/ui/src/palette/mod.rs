//! The command palette: a search box over the app's commands, styled with the house theme.
//!
//! The app gives the palette a list of [`PaletteEntry`] each frame (usually
//! [`PaletteEntry::from_registry`] plus whether each command is available right now) and runs
//! the [`CommandId`] that [`CommandPalette::show`] returns. Typing filters with a forgiving
//! fuzzy match (`sva` finds "Save As…"), arrow keys move, Enter runs, Escape or a click outside
//! closes. Recently run commands rank first among equal matches.

use egui::{Align2, Color32, CornerRadius, FontId, Id, Key, Modifiers, Order, Sense, Stroke, StrokeKind, TextEdit, vec2};

use crate::house::HouseTheme;
use crate::shortcuts::{CommandId, ShortcutRegistry};

#[cfg(test)]
mod tests;

/// How many matches the palette lists at most.
const MAX_ROWS: usize = 12;
/// How many recently run commands it remembers.
const MAX_RECENT: usize = 8;
const ROW_HEIGHT: f32 = 34.0;
const WIDTH: f32 = 560.0;

/// One command as the palette lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    pub id: CommandId,
    /// What the user searches and sees, such as `Save As…`.
    pub label: String,
    /// Shown dimmed before the label, such as `File`. May be empty.
    pub category: String,
    /// The shortcut as the platform shows it, such as `⇧⌘S`.
    pub shortcut: Option<String>,
    /// Unavailable commands are listed dimmed and cannot be run.
    pub enabled: bool,
}

impl PaletteEntry {
    /// One enabled entry per registered command, with its first shortcut.
    pub fn from_registry(registry: &ShortcutRegistry) -> Vec<Self> {
        registry
            .commands()
            .iter()
            .map(|c| Self {
                id: c.id.clone(),
                label: c.label.clone(),
                category: c.category.clone(),
                shortcut: registry.display(&c.id),
                enabled: true,
            })
            .collect()
    }
}

/// Scores how well `query` matches `text`, or `None` when it does not.
///
/// Every query character must appear in `text` in order, ignoring case and spaces in the query.
/// Matches at the start of words and runs of adjacent characters score higher; gaps and a
/// late first match score lower.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let wanted: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect();
    if wanted.is_empty() {
        return Some(0);
    }
    let hay: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let mut score = 0i32;
    let mut next = 0usize;
    let mut previous_match: Option<usize> = None;
    for (i, &c) in hay.iter().enumerate() {
        let Some(&want) = wanted.get(next) else { break };
        if c != want {
            continue;
        }
        let word_start = i == 0 || hay.get(i - 1).is_some_and(|p| !p.is_alphanumeric());
        score += 1;
        if word_start {
            score += 8;
        }
        match previous_match {
            Some(p) if p + 1 == i => score += 5,
            Some(p) => score -= i32::try_from(i - p - 1).unwrap_or(i32::MAX).min(10),
            None => score -= i32::try_from(i).unwrap_or(i32::MAX).min(20),
        }
        previous_match = Some(i);
        next += 1;
    }
    (next == wanted.len()).then_some(score)
}

/// The entries matching `query`, best first, as indexes into `entries`.
///
/// The label is matched first; the category is tried too (`file sa` finds File > Save). Ties
/// go to recently run commands, then to the original order. An empty query lists recent
/// commands first, then the rest in order.
pub fn matches(entries: &[PaletteEntry], query: &str, recent: &[CommandId]) -> Vec<usize> {
    let mut scored: Vec<(i32, usize, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let label = fuzzy_score(query, &entry.label);
            let with_category =
                if entry.category.is_empty() { None } else { fuzzy_score(query, &format!("{} {}", entry.category, entry.label)).map(|s| s - 4) };
            let score = label.max(with_category)?;
            let recency = recent.iter().position(|r| *r == entry.id).unwrap_or(usize::MAX);
            Some((score, recency, index))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    scored.into_iter().map(|(_, _, index)| index).collect()
}

/// The palette's state. Keep one in the app and call [`CommandPalette::show`] every frame.
#[derive(Clone, Debug, Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    selected: usize,
    recent: Vec<CommandId>,
    focus_pending: bool,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the palette with an empty search.
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.focus_pending = true;
    }

    /// Closes the palette.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Opens a closed palette and closes an open one (the usual shortcut behaviour).
    pub fn toggle(&mut self) {
        if self.open { self.close() } else { self.open() }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The current search text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Recently run commands, newest first.
    pub fn recent(&self) -> &[CommandId] {
        &self.recent
    }

    /// Records that `id` ran (also when it ran from a menu or shortcut), so it ranks first.
    pub fn note_used(&mut self, id: &CommandId) {
        self.recent.retain(|r| r != id);
        self.recent.insert(0, id.clone());
        self.recent.truncate(MAX_RECENT);
    }

    /// Draws the palette when open and returns the command the user chose this frame.
    pub fn show(&mut self, ctx: &egui::Context, theme: &HouseTheme, entries: &[PaletteEntry]) -> Option<CommandId> {
        if !self.open {
            return None;
        }

        // Keys the palette owns while open; taken before the text field sees them.
        let (escape, down, up, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if escape {
            self.close();
            return None;
        }

        let found = matches(entries, &self.query, &self.recent);
        let shown: Vec<&PaletteEntry> = found.iter().filter_map(|&i| entries.get(i)).take(MAX_ROWS).collect();
        if down && !shown.is_empty() {
            self.selected = (self.selected + 1) % shown.len();
        }
        if up && !shown.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(shown.len() - 1);
        }
        self.selected = self.selected.min(shown.len().saturating_sub(1));

        let mut chosen: Option<CommandId> = None;
        if enter && let Some(entry) = shown.get(self.selected).filter(|e| e.enabled) {
            chosen = Some(entry.id.clone());
        }

        let screen = ctx.content_rect();
        let width = WIDTH.min(screen.width() - 32.0).max(200.0);
        let top = screen.top() + (screen.height() * 0.12).clamp(24.0, 120.0);
        let area = egui::Area::new(Id::new("craft_ui_command_palette"))
            .order(Order::Foreground)
            .fixed_pos(egui::pos2(screen.center().x - width * 0.5, top))
            .show(ctx, |ui| {
                ui.set_width(width);
                theme.panel_frame().inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.set_width(width - 24.0);
                    let before = self.query.clone();
                    let edit = ui.add(
                        TextEdit::singleline(&mut self.query)
                            .id(Id::new("craft_ui_command_palette_query"))
                            .hint_text("Type a command")
                            .desired_width(f32::INFINITY)
                            .font(FontId::proportional(craft_tokens::font::SIZE_INPUT))
                            .margin(vec2(10.0, 8.0)),
                    );
                    if self.focus_pending {
                        edit.request_focus();
                        self.focus_pending = false;
                    }
                    if self.query != before {
                        self.selected = 0;
                    }
                    ui.add_space(6.0);

                    if shown.is_empty() {
                        ui.label(egui::RichText::new("No matching commands").color(theme.text_faint));
                    }
                    for (row, entry) in shown.iter().enumerate() {
                        if draw_row(ui, theme, entry, row == self.selected).clicked() && entry.enabled {
                            chosen = Some(entry.id.clone());
                        }
                    }
                });
            });

        // A click outside the palette dismisses it.
        let clicked_outside = ctx.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
        if clicked_outside && chosen.is_none() {
            self.close();
        }

        if let Some(id) = &chosen {
            self.note_used(id);
            self.close();
        }
        chosen
    }
}

fn draw_row(ui: &mut egui::Ui, theme: &HouseTheme, entry: &PaletteEntry, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, entry.enabled, &entry.label));
    let painter = ui.painter();
    let radius = CornerRadius::same(theme.radius_control);
    if selected {
        painter.rect(rect, radius, theme.accent_soft, Stroke::new(1.0, theme.accent_border), StrokeKind::Inside);
    } else if response.hovered() {
        painter.rect_filled(rect, radius, theme.hover);
    }
    let (label_color, category_color): (Color32, Color32) =
        if entry.enabled { (theme.text_strong, theme.text_muted) } else { (theme.text_faint, theme.text_faint) };
    let font = FontId::proportional(craft_tokens::font::SIZE_BODY);
    let mut x = rect.left() + 10.0;
    if !entry.category.is_empty() {
        let galley = painter.layout_no_wrap(format!("{}:", entry.category), font.clone(), category_color);
        let w = galley.size().x;
        painter.galley(egui::pos2(x, rect.center().y - galley.size().y * 0.5), galley, category_color);
        x += w + 6.0;
    }
    painter.text(egui::pos2(x, rect.center().y), Align2::LEFT_CENTER, &entry.label, font, label_color);
    if let Some(shortcut) = &entry.shortcut {
        painter.text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(craft_tokens::font::SIZE_META),
            theme.text_faint,
        );
    }
    response
}
