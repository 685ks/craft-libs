//! The house look: the tools-site design system (`craft-tokens`) applied to egui.
//!
//! [`HouseTheme`] resolves the web tokens into opaque egui colours and shapes. Apps copy what they
//! need into their own `Tokens` struct, call [`HouseTheme::apply`] to set egui's visuals, and use
//! the components here for the patterns the design system names: the framed panel and its header,
//! status boxes, chips, the primary button with its busy state, and loading skeletons.
//!
//! Translucent web surfaces are flattened onto the page colour, because egui cannot blur what is
//! behind a panel. Gold is reserved for "you / now / current", as the design system intends.

use std::f32::consts::TAU;

use craft_tokens::{color, motion, radius, size};
use egui::{Align2, Color32, CornerRadius, FontId, Frame, Margin, Rect, Response, Sense, Shape, Stroke, StrokeKind, Style, Ui, Vec2, pos2, vec2};

use crate::theme::WidgetPalette;

/// Status kinds, each drawn as text plus a tinted fill and a tinted border.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Ok,
    Warn,
    Error,
    /// Neutral information, in the accent colour.
    Info,
}

/// The house palette and shapes, resolved for egui.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HouseTheme {
    /// Page and canvas surround (`--bg`).
    pub page: Color32,
    /// Title bar, toolbars and sheets (`--bg-sheet`).
    pub chrome: Color32,
    /// Panel body: `--panel` flattened onto the page.
    pub panel: Color32,
    /// Inputs and tiles: `--fill-input` flattened onto the panel.
    pub field: Color32,
    /// Hover fill: `--fill-focus` flattened onto the panel.
    pub hover: Color32,
    /// Pressed fill, one step past hover.
    pub pressed: Color32,
    /// Panel frame and input borders (`--line`).
    pub line: Color32,
    /// Separators (`--line2`).
    pub line_soft: Color32,
    pub text: Color32,
    pub text_strong: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    /// Accent at 14%: selected rows, chip fills.
    pub accent_soft: Color32,
    /// Accent at 45%: focus and selection borders.
    pub accent_border: Color32,
    /// Text on the light primary button.
    pub on_accent: Color32,
    /// Reserved for "you / now / current".
    pub gold: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub error: Color32,
    pub scrim: Color32,
    /// Icon buttons and Gear inputs (10).
    pub radius_control: u8,
    /// Inputs, primary button, notes (12).
    pub radius_field: u8,
    /// Panels, stat tiles, sheets (14).
    pub radius_panel: u8,
    pub panel_shadow: egui::Shadow,
    pub sheet_shadow: egui::Shadow,
    /// Panel padding; 26 on desktop, 16 in compact layouts.
    pub panel_padding: i8,
    /// `--t-hover` in seconds; 0 when reduced motion is on.
    pub animation_time: f32,
}

impl Default for HouseTheme {
    fn default() -> Self {
        Self::new(false)
    }
}

impl HouseTheme {
    /// The house theme; `reduce_motion` turns every animation off, as the design system requires.
    pub fn new(reduce_motion: bool) -> Self {
        let page = color::BG_BASE;
        let panel = color::SURFACE_PANEL.over(page);
        let white = craft_tokens::Rgba::rgb(255, 255, 255);
        let c = |rgba: craft_tokens::Rgba| Color32::from(rgba);
        Self {
            page: c(page),
            chrome: c(color::BG_SHEET),
            panel: c(panel),
            field: c(color::SURFACE_FILL_3.over(panel)),
            hover: c(color::SURFACE_FILL_FOCUS.over(panel)),
            pressed: c(white.with_alpha(0.09).over(panel)),
            line: c(color::BORDER_LINE),
            line_soft: c(color::BORDER_LINE_SOFT),
            text: c(color::TEXT_INK),
            text_strong: c(color::TEXT_STRONG),
            text_muted: c(color::TEXT_MUTED),
            text_faint: c(color::TEXT_FAINT),
            accent: c(color::ACCENT_BASE),
            accent_soft: c(color::ACCENT_BASE.with_alpha(0.14)),
            accent_border: c(color::ACCENT_BASE.with_alpha(0.45)),
            on_accent: c(color::TEXT_ON_ACCENT),
            gold: c(color::ACCENT_GOLD),
            ok: c(color::STATUS_OK),
            warn: c(color::STATUS_WARN),
            error: c(color::STATUS_ERROR),
            scrim: c(color::SURFACE_SCRIM),
            radius_control: radius::LG as u8,
            radius_field: radius::XL as u8,
            radius_panel: radius::PANEL as u8,
            panel_shadow: craft_tokens::shadow::PANEL.into(),
            sheet_shadow: craft_tokens::shadow::SHEET.into(),
            panel_padding: craft_tokens::space::STEP_6 as i8,
            animation_time: if reduce_motion { 0.0 } else { motion::DURATION_HOVER.as_secs_f32() },
        }
    }

    /// The text colour of a status.
    pub fn status_color(&self, status: Status) -> Color32 {
        match status {
            Status::Ok => self.ok,
            Status::Warn => self.warn,
            Status::Error => self.error,
            Status::Info => self.accent,
        }
    }

    /// Set egui's visuals, widget fills and spacing to the house look. Fonts stay app-owned:
    /// load Inter from craft-fonts and set it as the proportional family.
    pub fn apply(&self, style: &mut Style) {
        let v = &mut style.visuals;
        *v = egui::Visuals::dark();
        v.override_text_color = Some(self.text);
        v.panel_fill = self.panel;
        v.window_fill = self.panel;
        v.extreme_bg_color = self.page;
        v.faint_bg_color = self.field;
        v.code_bg_color = self.field;
        v.window_stroke = Stroke::new(1.0, self.line);
        v.window_corner_radius = CornerRadius::same(self.radius_panel);
        v.menu_corner_radius = CornerRadius::same(self.radius_field);
        v.window_shadow = self.sheet_shadow;
        v.popup_shadow = self.panel_shadow;
        v.hyperlink_color = self.accent;
        v.warn_fg_color = self.warn;
        v.error_fg_color = self.error;
        v.selection.bg_fill = self.accent.gamma_multiply(0.3);
        v.selection.stroke = Stroke::new(1.0, self.accent);
        v.text_cursor.stroke = Stroke::new(2.0, self.accent);

        WidgetPalette { noninteractive: self.panel, inactive: self.field, hovered: self.hover, active: self.pressed, open: self.hover }.apply_to(
            &mut v.widgets,
            CornerRadius::same(self.radius_field),
            Stroke::new(1.0, self.text),
        );
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_muted);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.line_soft);
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, self.line);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, self.accent_border);
        v.widgets.active.bg_stroke = Stroke::new(1.0, self.accent);
        v.widgets.open.bg_stroke = Stroke::new(1.0, self.accent_border);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text_strong);
        v.widgets.active.fg_stroke = Stroke::new(1.0, self.text_strong);

        let s = &mut style.spacing;
        s.item_spacing = vec2(8.0, 6.0);
        s.button_padding = vec2(14.0, 8.0);
        s.window_margin = Margin::same(self.panel_padding);
        s.menu_margin = Margin::same(10);
        s.interact_size.y = 32.0;
        style.animation_time = self.animation_time;
    }

    /// The design system's panel: 1px `--line` frame, opaque glass body, 14px radius, soft shadow.
    pub fn panel_frame(&self) -> Frame {
        Frame::new()
            .fill(self.panel)
            .stroke(Stroke::new(1.0, self.line))
            .corner_radius(CornerRadius::same(self.radius_panel))
            .inner_margin(Margin::same(self.panel_padding))
            .shadow(self.panel_shadow)
    }

    /// A note or error box: status text colour, 6% tinted fill, 35% tinted border.
    pub fn status_frame(&self, status: Status) -> Frame {
        let c = self.status_color(status);
        Frame::new()
            .fill(c.gamma_multiply(0.06))
            .stroke(Stroke::new(1.0, c.gamma_multiply(0.35)))
            .corner_radius(CornerRadius::same(self.radius_field))
            .inner_margin(Margin::symmetric(14, 10))
    }
}

/// A panel header row: the 8px accent dot, a semibold title and an optional round `?` help button.
///
/// Returns the help button's response when one is shown; the caller opens its note on click.
pub fn panel_header(ui: &mut Ui, theme: &HouseTheme, title: &str, with_help: bool) -> Option<Response> {
    ui.horizontal(|ui| {
        let pin = size::PIN;
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(pin), Sense::hover());
        ui.painter().circle_filled(dot.center(), pin * 0.5, theme.accent);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(title).size(craft_tokens::font::SIZE_HEADING_2).strong().color(theme.text_strong));
        if !with_help {
            return None;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let d = size::HELP_BUTTON;
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(d), Sense::click());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Help"));
            let stroke = if response.hovered() { theme.accent_border } else { theme.line };
            ui.painter().circle(rect.center(), d * 0.5, theme.field, Stroke::new(1.0, stroke));
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "?", FontId::proportional(12.0), theme.text_muted);
            Some(response)
        })
        .inner
    })
    .inner
}

/// A pill with a coloured dot and an uppercase label, e.g. job state or sync state.
pub fn chip(ui: &mut Ui, theme: &HouseTheme, label: &str, status: Status) -> Response {
    let c = theme.status_color(status);
    let font = FontId::proportional(craft_tokens::font::SIZE_CHIP);
    let galley = ui.painter().layout_no_wrap(label.to_uppercase(), font, c);
    let pad = vec2(10.0, 4.0);
    let dot = 6.0;
    let size = vec2(pad.x * 2.0 + dot + 6.0 + galley.size().x, pad.y * 2.0 + galley.size().y.max(dot));
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label));
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(u8::MAX), c.gamma_multiply(0.07), Stroke::new(1.0, c.gamma_multiply(0.4)), StrokeKind::Inside);
    let dot_center = pos2(rect.left() + pad.x + dot * 0.5, rect.center().y);
    p.circle_filled(dot_center, dot * 0.5, c);
    p.galley(pos2(dot_center.x + dot * 0.5 + 6.0, rect.center().y - galley.size().y * 0.5), galley, c);
    response
}

/// The 50px primary button: light fill, dark text. While `busy` it ignores clicks, hides the
/// label and shows three hopping outlined diamonds.
pub fn primary_button(ui: &mut Ui, theme: &HouseTheme, label: &str, busy: bool) -> Response {
    let height = size::BUTTON_HEIGHT;
    let width = ui.available_width().clamp(120.0, 360.0);
    let sense = if busy { Sense::hover() } else { Sense::click() };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && !busy, label));
    let fill = if response.hovered() && !busy { Color32::WHITE } else { theme.text_strong };
    let shrink = if response.is_pointer_button_down_on() && !busy { 0.98 } else { 1.0 };
    let shown = Rect::from_center_size(rect.center(), rect.size() * shrink);
    let p = ui.painter();
    p.rect_filled(shown, CornerRadius::same(theme.radius_field), fill);
    if response.has_focus() {
        p.rect_stroke(
            shown.expand(3.0),
            CornerRadius::same(theme.radius_field + 3),
            Stroke::new(3.0, theme.accent.gamma_multiply(0.18)),
            StrokeKind::Outside,
        );
    }
    if busy {
        let t = ui.input(|i| i.time) as f32;
        for i in 0..3 {
            let phase = ((t * 1.1 - i as f32 * 0.15) * TAU).sin().max(0.0);
            let hop = if theme.animation_time > 0.0 { phase * 5.0 } else { 0.0 };
            let c = pos2(shown.center().x + (i as f32 - 1.0) * 16.0, shown.center().y - hop);
            let r = 4.5;
            let diamond = vec![pos2(c.x, c.y - r), pos2(c.x + r, c.y), pos2(c.x, c.y + r), pos2(c.x - r, c.y)];
            p.add(Shape::closed_line(diamond, Stroke::new(1.5, theme.on_accent)));
        }
        if theme.animation_time > 0.0 {
            ui.ctx().request_repaint();
        }
    } else {
        p.text(shown.center(), Align2::CENTER_CENTER, label, FontId::proportional(craft_tokens::font::SIZE_BUTTON), theme.on_accent);
    }
    response
}

/// An outlined placeholder with a soft accent shine passing over it, shown while content loads.
pub fn skeleton(ui: &mut Ui, theme: &HouseTheme, size: Vec2) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let radius = CornerRadius::same(theme.radius_field);
    let p = ui.painter();
    p.rect(rect, radius, theme.field, Stroke::new(1.0, theme.line_soft), StrokeKind::Inside);
    if theme.animation_time > 0.0 && rect.width() > 0.0 {
        let t = (ui.input(|i| i.time) as f32 / 1.6).fract();
        let band = (rect.width() * 0.3).max(24.0);
        let x = rect.left() - band + (rect.width() + band * 2.0) * motion::EASING_OUT.ease(t);
        let shine = Rect::from_min_max(pos2(x - band * 0.5, rect.top()), pos2(x + band * 0.5, rect.bottom())).intersect(rect);
        if shine.is_positive() {
            p.rect_filled(shine, radius, theme.accent.gamma_multiply(0.06));
        }
        ui.ctx().request_repaint();
    }
    response
}

#[cfg(test)]
mod tests;
