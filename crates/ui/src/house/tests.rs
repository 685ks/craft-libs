use super::*;
use egui::{Context, RawInput, Rect, pos2, vec2};

fn run(theme: &HouseTheme, add: impl FnMut(&mut Ui)) {
    let ctx = Context::default();
    ctx.global_style_mut(|s| theme.apply(s));
    let mut add = add;
    let mut output =
        ctx.run_ui(RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 400.0))), ..Default::default() }, |ui| add(ui));
    output.textures_delta.clear();
}

fn luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn surfaces_are_opaque_and_text_meets_contrast() {
    let t = HouseTheme::default();
    for surface in [t.page, t.chrome, t.panel, t.field, t.hover, t.pressed] {
        assert_eq!(surface.a(), 255);
    }
    assert_eq!(t.panel, Color32::from_rgb(12, 15, 21));
    for text in [t.text, t.text_strong, t.text_muted, t.text_faint, t.accent] {
        assert!(contrast(text, t.panel) >= 4.5, "{text:?} on panel");
        assert!(contrast(text, t.field) >= 4.5, "{text:?} on field");
    }
    assert!(contrast(t.on_accent, t.text_strong) >= 4.5);
}

#[test]
fn reduced_motion_turns_animation_off() {
    assert_eq!(HouseTheme::new(true).animation_time, 0.0);
    assert!((HouseTheme::new(false).animation_time - 0.2).abs() < 1e-6);
    let mut style = Style::default();
    HouseTheme::new(true).apply(&mut style);
    assert_eq!(style.animation_time, 0.0);
}

#[test]
fn apply_sets_house_visuals() {
    let t = HouseTheme::default();
    let mut style = Style::default();
    t.apply(&mut style);
    assert!(style.visuals.dark_mode);
    assert_eq!(style.visuals.panel_fill, t.panel);
    assert_eq!(style.visuals.widgets.inactive.bg_fill, t.field);
    assert_eq!(style.visuals.widgets.hovered.bg_stroke.color, t.accent_border);
    assert_eq!(style.visuals.window_corner_radius, CornerRadius::same(14));
    assert_eq!(style.spacing.window_margin, Margin::same(26));
}

#[test]
fn status_frames_tint_by_status() {
    let t = HouseTheme::default();
    let error = t.status_frame(Status::Error);
    assert_eq!(error.stroke.color, t.error.gamma_multiply(0.35));
    assert_ne!(t.status_frame(Status::Ok).fill, error.fill);
    assert_eq!(t.status_color(Status::Info), t.accent);
}

#[test]
fn busy_primary_button_does_not_click() {
    let t = HouseTheme::new(true);
    let mut sense = None;
    run(&t, |ui| sense = Some(primary_button(ui, &t, "Export", true).sense));
    assert!(!sense.unwrap().senses_click());
    run(&t, |ui| sense = Some(primary_button(ui, &t, "Export", false).sense));
    assert!(sense.unwrap().senses_click());
}

#[test]
fn components_allocate_their_size() {
    let t = HouseTheme::new(true);
    let mut rects = Vec::new();
    run(&t, |ui| {
        rects.push(primary_button(ui, &t, "Render", false).rect);
        rects.push(skeleton(ui, &t, vec2(200.0, 40.0)).rect);
        rects.push(chip(ui, &t, "Queued", Status::Warn).rect);
        let help = t.panel_frame().show(ui, |ui| panel_header(ui, &t, "Layers", true)).inner;
        rects.push(help.unwrap().rect);
        assert!(panel_header(ui, &t, "No help", false).is_none());
    });
    assert_eq!(rects[0].height(), 50.0);
    assert_eq!(rects[1].size(), vec2(200.0, 40.0));
    assert!(rects[2].width() > 40.0 && rects[2].height() >= 12.0);
    assert_eq!(rects[3].size(), vec2(24.0, 24.0));
}
