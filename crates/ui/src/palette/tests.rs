use super::*;
use crate::shortcuts::{Chord, Platform};
use egui::{Event, RawInput, Rect, pos2};

fn entries() -> (ShortcutRegistry, Vec<PaletteEntry>) {
    let mut r = ShortcutRegistry::new(Platform::Mac);
    r.register("file.save", "Save", "File", &[Chord::primary(Key::S)]).unwrap();
    r.register("file.save-as", "Save As…", "File", &[Chord::primary(Key::S).shift()]).unwrap();
    r.register("view.zoom-in", "Zoom In", "View", &[]).unwrap();
    r.register("edit.select-all", "Select All", "Edit", &[]).unwrap();
    let list = PaletteEntry::from_registry(&r);
    (r, list)
}

fn ids(list: &[PaletteEntry], found: &[usize]) -> Vec<String> {
    found.iter().map(|&i| list[i].id.as_str().to_owned()).collect()
}

#[test]
fn fuzzy_matches_subsequences_and_prefers_word_starts() {
    assert!(fuzzy_score("sva", "Save As…").is_some());
    assert!(fuzzy_score("xyz", "Save As…").is_none());
    assert_eq!(fuzzy_score("", "anything"), Some(0));
    assert!(fuzzy_score("SAVE", "save").is_some(), "case is ignored");
    assert!(fuzzy_score("save as", "Save As…").is_some(), "spaces in the query are ignored");
    let word_start = fuzzy_score("zi", "Zoom In").unwrap();
    let buried = fuzzy_score("zi", "Fizzing").unwrap();
    assert!(word_start > buried);
    let run = fuzzy_score("save", "Save").unwrap();
    let scattered = fuzzy_score("save", "Select All Views Everywhere").unwrap();
    assert!(run > scattered);
}

#[test]
fn matches_rank_and_fall_back_to_category() {
    let (_, list) = entries();
    assert_eq!(ids(&list, &matches(&list, "save", &[])), ["file.save", "file.save-as"]);
    assert_eq!(ids(&list, &matches(&list, "sva", &[])), ["file.save-as"]);
    assert_eq!(ids(&list, &matches(&list, "view zoom", &[])), ["view.zoom-in"]);
    assert!(matches(&list, "qqq", &[]).is_empty());
    assert_eq!(matches(&list, "", &[]).len(), list.len());
}

#[test]
fn recent_commands_win_ties_and_lead_an_empty_search() {
    let (_, list) = entries();
    let recent = vec![CommandId::new("edit.select-all").unwrap()];
    assert_eq!(ids(&list, &matches(&list, "", &recent))[0], "edit.select-all");
    let mut palette = CommandPalette::new();
    for id in ["a", "b", "a"] {
        palette.note_used(&CommandId::new(id).unwrap());
    }
    assert_eq!(palette.recent().iter().map(CommandId::as_str).collect::<Vec<_>>(), ["a", "b"]);
}

#[test]
fn entries_carry_platform_shortcuts() {
    let (_, list) = entries();
    assert_eq!(list[1].shortcut.as_deref(), Some("⇧⌘S"));
    assert_eq!(list[2].shortcut, None);
}

fn frame(ctx: &egui::Context, palette: &mut CommandPalette, list: &[PaletteEntry], events: Vec<Event>) -> Option<CommandId> {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))), events, ..Default::default() };
    let theme = HouseTheme::default();
    let mut chosen = None;
    let mut output = ctx.run_ui(input, |ui| chosen = palette.show(ui.ctx(), &theme, list));
    output.textures_delta.clear();
    chosen
}

fn key(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

#[test]
fn typing_and_enter_run_the_best_match() {
    let ctx = egui::Context::default();
    let (_, list) = entries();
    let mut palette = CommandPalette::new();
    assert_eq!(frame(&ctx, &mut palette, &list, vec![]), None, "closed palettes draw nothing");
    palette.open();
    frame(&ctx, &mut palette, &list, vec![]);
    frame(&ctx, &mut palette, &list, vec![Event::Text("sva".into())]);
    assert_eq!(palette.query(), "sva");
    let chosen = frame(&ctx, &mut palette, &list, vec![key(Key::Enter)]);
    assert_eq!(chosen.as_ref().map(CommandId::as_str), Some("file.save-as"));
    assert!(!palette.is_open());
    assert_eq!(palette.recent().first(), chosen.as_ref());
}

#[test]
fn arrows_move_the_selection_and_wrap() {
    let ctx = egui::Context::default();
    let (_, list) = entries();
    let mut palette = CommandPalette::new();
    palette.open();
    frame(&ctx, &mut palette, &list, vec![]);
    frame(&ctx, &mut palette, &list, vec![key(Key::ArrowDown)]);
    let chosen = frame(&ctx, &mut palette, &list, vec![key(Key::Enter)]);
    assert_eq!(chosen.map(|c| c.as_str().to_owned()).as_deref(), Some("file.save-as"));

    palette.open();
    frame(&ctx, &mut palette, &list, vec![key(Key::ArrowUp)]);
    let chosen = frame(&ctx, &mut palette, &list, vec![key(Key::Enter)]);
    assert_eq!(chosen.map(|c| c.as_str().to_owned()).as_deref(), Some("edit.select-all"), "up from the top wraps to the end");
}

#[test]
fn escape_closes_and_disabled_entries_do_not_run() {
    let ctx = egui::Context::default();
    let (_, mut list) = entries();
    for entry in &mut list {
        entry.enabled = false;
    }
    let mut palette = CommandPalette::new();
    palette.open();
    frame(&ctx, &mut palette, &list, vec![]);
    assert_eq!(frame(&ctx, &mut palette, &list, vec![key(Key::Enter)]), None);
    assert!(palette.is_open());
    frame(&ctx, &mut palette, &list, vec![key(Key::Escape)]);
    assert!(!palette.is_open());
    palette.toggle();
    assert!(palette.is_open());
    palette.toggle();
    assert!(!palette.is_open());
}

#[test]
fn an_empty_list_and_a_tiny_screen_do_not_panic() {
    let ctx = egui::Context::default();
    let mut palette = CommandPalette::new();
    palette.open();
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0))), ..Default::default() };
    let theme = HouseTheme::new(true);
    let mut output = ctx.run_ui(input, |ui| {
        palette.show(ui.ctx(), &theme, &[]);
    });
    output.textures_delta.clear();
    assert_eq!(frame(&ctx, &mut palette, &[], vec![key(Key::Enter), key(Key::ArrowDown)]), None);
}
