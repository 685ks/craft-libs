use super::*;
use egui::{Event, RawInput};

fn chord(text: &str) -> Chord {
    Chord::parse(text).unwrap()
}

fn registry(platform: Platform) -> (ShortcutRegistry, CommandId, CommandId, CommandId) {
    let mut r = ShortcutRegistry::new(platform);
    let save = r.register("file.save", "Save", "File", &[chord("Mod+S")]).unwrap();
    let save_as = r.register("file.save-as", "Save As…", "File", &[chord("Mod+Shift+S")]).unwrap();
    let palette = r.register("view.palette", "Command Palette", "View", &[chord("Mod+Shift+P"), chord("F1")]).unwrap();
    (r, save, save_as, palette)
}

#[test]
fn chords_parse_in_many_spellings() {
    assert_eq!(chord("Mod+S"), Chord::primary(Key::S));
    assert_eq!(chord("cmd + shift + z"), Chord::primary(Key::Z).shift());
    assert_eq!(chord("Command+Option+I"), Chord::primary(Key::I).alt());
    assert_eq!(chord("Ctrl+Alt+Delete"), Chord::key(Key::Delete).ctrl().alt());
    assert_eq!(chord("F5"), Chord::key(Key::F5));
    assert_eq!(chord("enter"), Chord::key(Key::Enter));
    assert_eq!(chord("Mod++"), Chord::primary(Key::Plus));
    assert_eq!(chord("Mod+Comma"), Chord::primary(Key::Comma));
    assert_eq!(chord("Mod+,"), Chord::primary(Key::Comma));
    assert_eq!(chord("⇧+⌘+Z"), Chord::primary(Key::Z).shift());
}

#[test]
fn bad_chords_are_errors_not_panics() {
    assert_eq!(Chord::parse("  "), Err(ChordError::Empty));
    assert_eq!(Chord::parse("Mod+Shift"), Err(ChordError::NoKey));
    assert_eq!(Chord::parse("A+B"), Err(ChordError::TwoKeys));
    assert_eq!(Chord::parse("Mod+Banana"), Err(ChordError::UnknownKey("Banana".into())));
    for odd in ["+++", "++", "+", "Mod+", "Shift++", "\u{0}", "⌘⌘⌘"] {
        let _ = Chord::parse(odd);
    }
    assert_eq!(Chord::parse("+"), Ok(Chord::key(Key::Plus)));
}

#[test]
fn portable_form_round_trips() {
    for text in ["Mod+S", "Mod+Ctrl+Alt+Shift+F", "F12", "Alt+Enter", "Mod+Plus"] {
        let c = chord(text);
        assert_eq!(chord(&c.portable()), c, "{text}");
    }
    assert_eq!(chord("shift+cmd+z").portable(), "Mod+Shift+Z");
    assert_eq!("Mod+K".parse::<Chord>(), Ok(Chord::primary(Key::K)));
}

#[test]
fn display_follows_the_platform() {
    let redo = chord("Mod+Shift+Z");
    assert_eq!(redo.display(Platform::Mac), "⇧⌘Z");
    assert_eq!(redo.display(Platform::Other), "Ctrl+Shift+Z");
    assert_eq!(chord("Ctrl+Alt+Tab").display(Platform::Mac), "⌃⌥Tab");
    assert_eq!(chord("Mod+Ctrl+F").display(Platform::Other), "Ctrl+F");
}

#[test]
fn mod_and_ctrl_conflict_only_off_mac() {
    for (platform, expect_conflict) in [(Platform::Mac, false), (Platform::Other, true)] {
        let mut r = ShortcutRegistry::new(platform);
        r.register("a", "A", "", &[chord("Mod+K")]).unwrap();
        r.register("b", "B", "", &[chord("Ctrl+K")]).unwrap();
        assert_eq!(!r.conflicts().is_empty(), expect_conflict, "{platform:?}");
    }
}

#[test]
fn conflicts_name_both_commands() {
    let (mut r, save, _, _) = registry(Platform::Other);
    assert!(r.conflicts().is_empty());
    let print = r.register("file.print", "Print", "File", &[chord("Ctrl+S")]).unwrap();
    let conflicts = r.conflicts();
    assert_eq!(conflicts, vec![Conflict { chord: chord("Mod+S"), first: save, second: print }]);
}

#[test]
fn registration_rejects_bad_and_duplicate_ids() {
    let (mut r, save, _, _) = registry(Platform::Mac);
    assert_eq!(r.register("file.save", "Again", "", &[]), Err(RegistryError::Duplicate(save)));
    assert!(matches!(r.register("", "Empty", "", &[]), Err(RegistryError::BadId(_))));
    assert!(matches!(r.register("has space", "X", "", &[]), Err(RegistryError::BadId(_))));
    assert!(matches!(r.register(&"x".repeat(129), "X", "", &[]), Err(RegistryError::BadId(_))));
    let twice = r.register("dup.defaults", "D", "", &[chord("F2"), chord("F2")]).unwrap();
    assert_eq!(r.bindings(&twice), &[chord("F2")]);
}

#[test]
fn rebind_refuses_taken_chords_and_changes_nothing() {
    let (mut r, save, save_as, _) = registry(Platform::Mac);
    let err = r.rebind(&save_as, &[chord("Cmd+S")]);
    assert_eq!(err, Err(RegistryError::Taken { chord: chord("Mod+S"), by: save.clone() }));
    assert_eq!(r.bindings(&save_as), &[chord("Mod+Shift+S")]);
    assert!(!r.is_overridden(&save_as));
    // Rebinding a command to its own chord is fine.
    assert_eq!(r.rebind(&save, &[chord("Mod+S"), chord("F2")]), Ok(()));
    assert_eq!(r.bindings(&save), &[chord("Mod+S"), chord("F2")]);
    assert!(r.is_overridden(&save));
    let unknown = CommandId::new("nope").unwrap();
    assert_eq!(r.rebind(&unknown, &[]), Err(RegistryError::Unknown(unknown)));
}

#[test]
fn rebind_taking_moves_the_chord() {
    let (mut r, save, save_as, palette) = registry(Platform::Mac);
    let losers = r.rebind_taking(&save_as, &[chord("Mod+S")]).unwrap();
    assert_eq!(losers, vec![save.clone()]);
    assert_eq!(r.bindings(&save), &[] as &[Chord]);
    assert_eq!(r.command_for(chord("Mod+S")), Some(&save_as));
    assert!(r.conflicts().is_empty());
    // Taking one of two chords leaves the other.
    r.rebind_taking(&save, &[chord("F1")]).unwrap();
    assert_eq!(r.bindings(&palette), &[chord("Mod+Shift+P")]);
}

#[test]
fn reset_restores_defaults_and_matching_defaults_is_not_an_override() {
    let (mut r, save, save_as, _) = registry(Platform::Other);
    r.rebind(&save, &[chord("F2")]).unwrap();
    r.rebind(&save_as, &[]).unwrap();
    assert_eq!(r.overrides().len(), 2);
    r.reset(&save);
    assert_eq!(r.bindings(&save), &[chord("Mod+S")]);
    r.rebind(&save_as, &[chord("Mod+Shift+S")]).unwrap();
    assert!(!r.is_overridden(&save_as), "binding back to the defaults clears the override");
    r.rebind(&save, &[]).unwrap();
    r.reset_all();
    assert!(r.overrides().is_empty());
}

#[test]
fn overrides_round_trip_and_tolerate_bad_entries() {
    let (mut r, save, save_as, _) = registry(Platform::Mac);
    r.rebind(&save, &[chord("Mod+Alt+S")]).unwrap();
    r.rebind(&save_as, &[]).unwrap();
    let saved = r.overrides();
    assert_eq!(saved.get("file.save"), Some(&vec!["Mod+Alt+S".to_owned()]));

    let (mut fresh, _, _, _) = registry(Platform::Mac);
    assert!(fresh.apply_overrides(&saved).is_empty());
    assert_eq!(fresh.bindings(&save), r.bindings(&save));
    assert_eq!(fresh.bindings(&save_as), &[] as &[Chord]);

    let mut messy = BTreeMap::new();
    messy.insert("gone.command".to_owned(), vec!["F3".to_owned()]);
    messy.insert("file.save".to_owned(), vec!["Mod+Banana".to_owned(), "F4".to_owned()]);
    let problems = fresh.apply_overrides(&messy);
    assert_eq!(problems.len(), 2);
    assert!(problems.contains(&OverrideProblem::UnknownCommand("gone.command".into())));
    assert_eq!(fresh.bindings(&save), &[chord("F4")]);
}

#[test]
fn display_uses_the_first_binding() {
    let (r, save, _, palette) = registry(Platform::Mac);
    assert_eq!(r.display(&save).as_deref(), Some("⌘S"));
    assert_eq!(r.display(&palette).as_deref(), Some("⇧⌘P"));
    assert_eq!(r.find("view.palette").map(|c| c.label.as_str()), Some("Command Palette"));
    assert_eq!(r.find("file.save").map(|c| c.defaults().len()), Some(1));
}

#[test]
fn platform_from_egui() {
    assert_eq!(Platform::from_egui(egui::os::OperatingSystem::Mac), Platform::Mac);
    assert_eq!(Platform::from_egui(egui::os::OperatingSystem::Windows), Platform::Other);
    assert_eq!(Platform::from_egui(egui::os::OperatingSystem::Nix), Platform::Other);
}

fn press(ctx: &egui::Context, key: Key, modifiers: Modifiers, registry: &ShortcutRegistry) -> Vec<CommandId> {
    let input = RawInput { events: vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }], ..Default::default() };
    let mut fired = Vec::new();
    let mut output = ctx.run_ui(input, |ui| fired = ui.ctx().input_mut(|i| registry.consume(i)));
    output.textures_delta.clear();
    fired
}

#[test]
fn consume_fires_the_most_specific_command_once() {
    let ctx = egui::Context::default();
    let (r, save, save_as, _) = registry(Platform::Other);
    let ctrl_shift = Modifiers { ctrl: true, command: true, shift: true, ..Default::default() };
    assert_eq!(press(&ctx, Key::S, ctrl_shift, &r), vec![save_as]);
    let ctrl = Modifiers { ctrl: true, command: true, ..Default::default() };
    assert_eq!(press(&ctx, Key::S, ctrl, &r), vec![save]);
    assert!(press(&ctx, Key::S, Modifiers::default(), &r).is_empty());
}

#[test]
fn consume_on_mac_needs_cmd_not_ctrl() {
    let ctx = egui::Context::default();
    let (r, save, _, _) = registry(Platform::Mac);
    let cmd = Modifiers { mac_cmd: true, command: true, ..Default::default() };
    assert_eq!(press(&ctx, Key::S, cmd, &r), vec![save]);
    let ctrl = Modifiers { ctrl: true, ..Default::default() };
    assert!(press(&ctx, Key::S, ctrl, &r).is_empty());
}
