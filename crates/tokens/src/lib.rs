#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

//! House design tokens for the Craft apps.
//!
//! `tokens.json` (W3C design-token format) is the source of truth; `src/generated.rs` is
//! produced from it by `cargo test -p craft-tokens` with `CRAFT_TOKENS_REGENERATE=1`, and the
//! same test fails when the two drift apart. The values come from a web design system, so this
//! crate keeps them as plain data: colours, pixel sizes, durations and easing curves. CSS-only
//! values (viewport clamps, blur, `vh`) are reduced to their desktop end or left out; see
//! `ADOPTION.md` in the design-system folder for how each one maps to egui.
//!
//! The `egui` feature adds conversions to egui types.

use std::time::Duration;

/// An sRGB colour with straight (unmultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// The same colour with alpha `fraction` (0..=1) of full opacity, e.g. a 14% accent fill.
    pub fn with_alpha(self, fraction: f32) -> Self {
        Self { a: (fraction.clamp(0.0, 1.0) * 255.0).round() as u8, ..self }
    }

    /// This colour composited over an opaque `base`, for surfaces that are translucent on the web
    /// but must be opaque where nothing behind them can be blurred.
    pub fn over(self, base: Rgba) -> Rgba {
        let a = f32::from(self.a) / 255.0;
        let mix = |top: u8, bottom: u8| (f32::from(top) * a + f32::from(bottom) * (1.0 - a)).round() as u8;
        Rgba::rgb(mix(self.r, base.r), mix(self.g, base.g), mix(self.b, base.b))
    }
}

/// A CSS box shadow: offset, blur, spread and colour, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Rgba,
}

/// A CSS `cubic-bezier(x1, y1, x2, y2)` easing curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier(pub f32, pub f32, pub f32, pub f32);

impl CubicBezier {
    /// Eased progress for linear progress `t` in 0..=1.
    pub fn ease(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let CubicBezier(x1, y1, x2, y2) = self;
        let bezier = |p1: f32, p2: f32, s: f32| 3.0 * (1.0 - s).powi(2) * s * p1 + 3.0 * (1.0 - s) * s * s * p2 + s.powi(3);
        // Solve x(s) = t by bisection; x is monotonic because x1 and x2 are within 0..=1.
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..32 {
            let mid = (lo + hi) * 0.5;
            if bezier(x1, x2, mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        bezier(y1, y2, (lo + hi) * 0.5)
    }
}

const fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

#[rustfmt::skip]
mod generated;
pub use generated::*;

/// The UI font stack, first choice first. Inter ships in craft-fonts; the rest are fallbacks.
pub const FONT_FAMILY: &[&str] = &["Inter", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "sans-serif"];

#[cfg(feature = "egui")]
mod egui_impl {
    use super::{Rgba, Shadow};

    impl Rgba {
        /// This colour as an egui colour, usable in `const` items.
        pub const fn color32(self) -> egui::Color32 {
            egui::Color32::from_rgba_unmultiplied_const(self.r, self.g, self.b, self.a)
        }
    }

    impl From<Rgba> for egui::Color32 {
        fn from(c: Rgba) -> Self {
            egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
        }
    }

    impl From<Shadow> for egui::Shadow {
        fn from(s: Shadow) -> Self {
            let i8_of = |v: f32| v.round().clamp(-128.0, 127.0) as i8;
            let u8_of = |v: f32| v.round().clamp(0.0, 255.0) as u8;
            egui::Shadow { offset: [i8_of(s.x), i8_of(s.y)], blur: u8_of(s.blur), spread: u8_of(s.spread.max(0.0)), color: s.color.into() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over_flattens_the_panel_onto_the_page() {
        assert_eq!(color::SURFACE_PANEL.over(color::BG_BASE), Rgba::rgb(12, 15, 21));
    }

    #[test]
    fn with_alpha_rounds_to_the_nearest_step() {
        assert_eq!(color::ACCENT_BASE.with_alpha(0.14).a, 36);
        assert_eq!(color::ACCENT_BASE.with_alpha(2.0).a, 255);
    }

    #[test]
    fn ease_out_is_monotonic_and_pinned_at_the_ends() {
        let curve = motion::EASING_OUT;
        assert!(curve.ease(0.0).abs() < 1e-4);
        assert!((curve.ease(1.0) - 1.0).abs() < 1e-4);
        let samples: Vec<f32> = (0..=20).map(|i| curve.ease(i as f32 / 20.0)).collect();
        assert!(samples.windows(2).all(|w| w[1] >= w[0]));
        // The house curve front-loads motion: half the time covers well over half the distance.
        assert!(curve.ease(0.5) > 0.8);
    }

    #[cfg(feature = "egui")]
    #[test]
    fn color32_matches_from() {
        const ACCENT: egui::Color32 = color::ACCENT_BASE.color32();
        assert_eq!(ACCENT, egui::Color32::from(color::ACCENT_BASE));
        assert_eq!(color::BORDER_LINE.color32(), egui::Color32::from(color::BORDER_LINE));
    }

    #[test]
    fn spring_overshoots() {
        assert!((0..=100).map(|i| motion::EASING_SPRING.ease(i as f32 / 100.0)).fold(0.0, f32::max) > 1.0);
    }
}
