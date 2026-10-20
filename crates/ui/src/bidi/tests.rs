use super::*;
use crate::layout::{SplitAxis, SplitSize, split_rect};
use egui::{Pos2, RawInput, pos2, vec2};

fn within() -> Rect {
    Rect::from_min_max(pos2(10.0, 20.0), pos2(410.0, 220.0))
}

#[test]
fn left_to_right_text_is_borrowed_unchanged() {
    for text in ["", "Save", "Save as… (Ctrl+Shift+S)", "naïve café 12:30"] {
        assert!(matches!(display(text, None), Cow::Borrowed(t) if t == text), "{text}");
        assert!(matches!(display(text, Some(Direction::Ltr)), Cow::Borrowed(t) if t == text), "{text}");
    }
    assert!(matches!(display("Save", Some(Direction::Rtl)), Cow::Borrowed("Save")));
}

#[test]
fn hebrew_is_reversed_into_visual_order() {
    assert_eq!(display("שלום", None), "םולש");
    assert_eq!(display("שלום עולם", Some(Direction::Rtl)), "םלוע םולש");
}

#[test]
fn hebrew_inside_an_english_label_reverses_only_the_hebrew() {
    assert_eq!(display("Open קובץ חדש now", Some(Direction::Ltr)), "Open שדח ץבוק now");
    assert_eq!(display("Open קובץ חדש now", None), "Open שדח ץבוק now");
}

#[test]
fn latin_inside_a_right_to_left_label_keeps_its_order_and_brackets_mirror() {
    assert_eq!(display("שמירה (Ctrl+S)", Some(Direction::Rtl)), "(Ctrl+S) הרימש");
    assert_eq!(display("[שמירה]", Some(Direction::Rtl)), "[הרימש]");
    assert_eq!(display("«שלום»", Some(Direction::Rtl)), "«םולש»");
}

#[test]
fn arabic_numbers_stay_left_to_right() {
    assert_eq!(display("صفحة 3 من 10", Some(Direction::Rtl)), "10 نم 3 ةحفص");
    // After Arabic letters digits are Arabic numbers (rule W2), so the % sign joins the
    // right-to-left run and is drawn to their left, as browsers show it.
    assert_eq!(display("مستوى 125%", Some(Direction::Rtl)), "%125 ىوتسم");
    assert_eq!(display("Zoom 125%", Some(Direction::Rtl)), "Zoom 125%");
}

#[test]
fn an_isolated_file_name_cannot_reorder_the_sentence() {
    // "Save as {name}?" with a Latin name, as craft_i18n::fmt_isolated builds it.
    let text = "حفظ باسم \u{2068}report.pdf\u{2069}؟";
    assert_eq!(display(text, Some(Direction::Rtl)), "؟report.pdf مساب ظفح");
    // Without the isolate a leading digit run in the name would join the sentence's numbers;
    // with it the name stays one left-to-right unit and the marks are gone.
    let shown = display("\u{2068}2024 report\u{2069} :ملف", Some(Direction::Rtl));
    assert_eq!(shown, "فلم: 2024 report");
    assert!(!shown.chars().any(is_bidi_control));
}

#[test]
fn marks_are_applied_then_removed() {
    // An RLM after the bracket makes it part of the Hebrew run.
    assert_eq!(display("abc שלום!\u{200F}", Some(Direction::Ltr)), "abc !םולש");
    assert_eq!(display("\u{200E}abc", None), "abc");
}

#[test]
fn combining_marks_stay_after_their_letter() {
    // shin + qamats + shin dot, lamed, vav + holam, final mem.
    assert_eq!(display("\u{5E9}\u{5B8}\u{5C1}\u{5DC}\u{5D5}\u{5B9}\u{5DD}", None), "\u{5DD}\u{5D5}\u{5B9}\u{5DC}\u{5E9}\u{5B8}\u{5C1}");
    // beh + fatha, teh + kasra.
    assert_eq!(display("\u{628}\u{64E}\u{62A}\u{650}", None), "\u{62A}\u{650}\u{628}\u{64E}");
}

#[test]
fn each_line_is_its_own_paragraph() {
    assert_eq!(display("abc\nשלום\n\nאב", None), "abc\nםולש\n\nבא");
    assert_eq!(display("שלום\nabc", None), "םולש\nabc");
}

#[test]
fn base_direction_of_text() {
    assert_eq!(Direction::of_text("שלום abc"), Some(Direction::Rtl));
    assert_eq!(Direction::of_text("123 مرحبا"), Some(Direction::Rtl));
    assert_eq!(Direction::of_text("abc שלום"), Some(Direction::Ltr));
    assert_eq!(Direction::of_text("123 ()"), None);
    assert_eq!(Direction::of_text(""), None);
}

#[test]
fn direction_basics() {
    assert_eq!(Direction::from_rtl(true), Direction::Rtl);
    assert_eq!(Direction::from_rtl(false), Direction::default());
    assert_eq!((Direction::Ltr.leading(), Direction::Ltr.trailing()), (Align::Min, Align::Max));
    assert_eq!((Direction::Rtl.leading(), Direction::Rtl.trailing()), (Align::Max, Align::Min));
    assert_eq!(Direction::Rtl.row(Align::Center), Layout::right_to_left(Align::Center));
    assert_eq!(Direction::Ltr.row(Align::Center), Layout::left_to_right(Align::Center));
    assert_eq!(Direction::Rtl.logical_key(Key::ArrowLeft), Key::ArrowRight);
    assert_eq!(Direction::Rtl.logical_key(Key::ArrowRight), Key::ArrowLeft);
    assert_eq!(Direction::Rtl.logical_key(Key::ArrowUp), Key::ArrowUp);
    assert_eq!(Direction::Ltr.logical_key(Key::ArrowLeft), Key::ArrowLeft);
    assert_eq!((Direction::Rtl.logical_dx(5.0), Direction::Ltr.logical_dx(5.0)), (-5.0, 5.0));
}

#[test]
fn rectangles_mirror_inside_their_container() {
    let r = Rect::from_min_max(pos2(20.0, 30.0), pos2(60.0, 50.0));
    let m = Direction::Rtl.mirror_rect(r, within());
    assert_eq!(m, Rect::from_min_max(pos2(360.0, 30.0), pos2(400.0, 50.0)));
    assert_eq!(Direction::Rtl.mirror_rect(m, within()), r, "mirroring twice is the identity");
    assert_eq!(Direction::Ltr.mirror_rect(r, within()), r);
    assert_eq!(Direction::Rtl.mirror_x(10.0, within()), 410.0);
    assert_eq!(Direction::Rtl.mirror_x(210.0, within()), 210.0);
}

#[test]
fn mirrored_splits_put_the_first_pane_on_the_right() {
    let layout = split_rect(within(), SplitAxis::Horizontal, SplitSize::FixedFirst(100.0), 4.0, 20.0).unwrap();
    let mirrored = layout.mirrored(within());
    assert_eq!(mirrored.first, Rect::from_min_max(pos2(310.0, 20.0), pos2(410.0, 220.0)));
    assert_eq!(mirrored.divider, Rect::from_min_max(pos2(306.0, 20.0), pos2(310.0, 220.0)));
    assert_eq!(mirrored.second, Rect::from_min_max(pos2(10.0, 20.0), pos2(306.0, 220.0)));
    assert_eq!((mirrored.first_size(), mirrored.available()), (layout.first_size(), layout.available()));
    // Dragging the mirrored divider 20 points to the left grows the first pane.
    let resized = mirrored.resized(SplitSize::FixedFirst(100.0), Direction::Rtl.logical_dx(-20.0), 20.0);
    assert_eq!(resized, Some(SplitSize::FixedFirst(120.0)));
    let vertical = split_rect(within(), SplitAxis::Vertical, SplitSize::Ratio(0.5), 2.0, 0.0).unwrap();
    assert_eq!(vertical.mirrored(within()), vertical, "a vertical split spans the width, so it is unchanged");
}

fn row_positions(direction: Direction) -> (f32, f32) {
    let ctx = egui::Context::default();
    let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 100.0))), ..Default::default() };
    let mut xs = (0.0, 0.0);
    let mut output = ctx.run_ui(input, |ui| {
        horizontal(ui, direction, |ui| {
            let first = ui.label("first").rect;
            let second = ui.label("second").rect;
            xs = (first.center().x, second.center().x);
        });
    });
    output.textures_delta.clear();
    xs
}

#[test]
fn rows_start_at_the_leading_edge() {
    let (first, second) = row_positions(Direction::Ltr);
    assert!(first < second && first < 200.0, "{first} {second}");
    let (first, second) = row_positions(Direction::Rtl);
    assert!(first > second && first > 200.0, "{first} {second}");
}
