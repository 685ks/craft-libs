// @generated from tokens.json by tests/generated_is_current.rs. Do not edit by hand.

pub mod blur {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `blur.button`: `8px`
    pub const BUTTON: f32 = 8.0;

    /// `blur.gate`: `12px`
    pub const GATE: f32 = 12.0;

    /// `blur.haze`: `70px`
    pub const HAZE: f32 = 70.0;

    /// `blur.panel`: `14px`
    pub const PANEL: f32 = 14.0;

    /// `blur.scrim`: `3px`
    pub const SCRIM: f32 = 3.0;
}

pub mod border {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `border.width`: `1px`
    ///
    /// panels draw their edge as 1px padding around the inner box (background: --line)
    pub const WIDTH: f32 = 1.0;
}

pub mod breakpoint {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `breakpoint.desktop`: `701px`
    ///
    /// min-width: long scroll between stops, no snap
    pub const DESKTOP: f32 = 701.0;

    /// `breakpoint.gear-phone-input`: `560px`
    pub const GEAR_PHONE_INPUT: f32 = 560.0;

    /// `breakpoint.gear-stage`: `660px`
    ///
    /// Gear: slots arranged around the figure
    pub const GEAR_STAGE: f32 = 660.0;

    /// `breakpoint.phone`: `700px`
    ///
    /// max-width: stacked forms, centred stops, bottom-anchored panels, mandatory snap scrolling
    pub const PHONE: f32 = 700.0;
}

pub mod color {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `color.accent.base`: `#8fb8d8`
    ///
    /// --neon (and --violet); steel blue: pins, focus, links, favicon
    pub const ACCENT_BASE: Rgba = Rgba::rgb(143, 184, 216);

    /// `color.accent.bright`: `#b5d3ea`
    ///
    /// Gear page --ks-accent-bright
    pub const ACCENT_BRIGHT: Rgba = Rgba::rgb(181, 211, 234);

    /// `color.accent.gold`: `#d9c08a`
    ///
    /// --gold; Truegold, the player's castle, 'now' line, crits
    pub const ACCENT_GOLD: Rgba = Rgba::rgb(217, 192, 138);

    /// `color.accent.gold-text`: `#f3e6c4`
    ///
    /// text on a gold-tinted calendar day
    pub const ACCENT_GOLD_TEXT: Rgba = Rgba::rgb(243, 230, 196);

    /// `color.accent.high`: `#e6eef5`
    ///
    /// --neon-hi
    pub const ACCENT_HIGH: Rgba = Rgba::rgb(230, 238, 245);

    /// `color.accent.low`: `#3d5a73`
    ///
    /// --neon-lo; start of the progress-bar gradient
    pub const ACCENT_LOW: Rgba = Rgba::rgb(61, 90, 115);

    /// `color.accent.soft`: `#9fb0c0`
    ///
    /// --pink (named pink, used as a soft steel)
    pub const ACCENT_SOFT: Rgba = Rgba::rgb(159, 176, 192);

    /// `color.bg.avatar`: `#1a2230`
    pub const BG_AVATAR: Rgba = Rgba::rgb(26, 34, 48);

    /// `color.bg.base`: `#07090d`
    ///
    /// --bg; page background and theme-color
    pub const BG_BASE: Rgba = Rgba::rgb(7, 9, 13);

    /// `color.bg.map-node`: `#0c0f16`
    ///
    /// canvas fill of event circles on the map
    pub const BG_MAP_NODE: Rgba = Rgba::rgb(12, 15, 22);

    /// `color.bg.map-tile`: `rgba(20,27,38,.55)`
    ///
    /// canvas fill of the kingdom map
    pub const BG_MAP_TILE: Rgba = Rgba::new(20, 27, 38, 140);

    /// `color.bg.sheet`: `#0b0e14`
    ///
    /// full-screen pop-up body; also Gear page --bg-1
    pub const BG_SHEET: Rgba = Rgba::rgb(11, 14, 20);

    /// `color.bg.sky-bottom`: `#0a0d13`
    ///
    /// bottom of the fixed backdrop gradient
    pub const BG_SKY_BOTTOM: Rgba = Rgba::rgb(10, 13, 19);

    /// `color.bg.sky-glow`: `#111823`
    ///
    /// radial glow behind the map, 80% x 60% at 50% 60%
    pub const BG_SKY_GLOW: Rgba = Rgba::rgb(17, 24, 35);

    /// `color.bg.sky-top`: `#06080c`
    ///
    /// top of the fixed backdrop gradient
    pub const BG_SKY_TOP: Rgba = Rgba::rgb(6, 8, 12);

    /// `color.border.gear`: `rgba(255,255,255,.10)`
    ///
    /// Gear page --border
    pub const BORDER_GEAR: Rgba = Rgba::new(255, 255, 255, 26);

    /// `color.border.gear-soft`: `rgba(255,255,255,.06)`
    ///
    /// Gear page --border-soft
    pub const BORDER_GEAR_SOFT: Rgba = Rgba::new(255, 255, 255, 15);

    /// `color.border.line`: `rgba(255,255,255,.12)`
    ///
    /// --line; the 1px edge of every panel, input and chip
    pub const BORDER_LINE: Rgba = Rgba::new(255, 255, 255, 31);

    /// `color.border.line-soft`: `rgba(255,255,255,.08)`
    ///
    /// --line2; dividers inside panels
    pub const BORDER_LINE_SOFT: Rgba = Rgba::new(255, 255, 255, 20);

    /// `color.map.city`: `#7d8ea0`
    ///
    /// other players' cities
    pub const MAP_CITY: Rgba = Rgba::rgb(125, 142, 160);

    /// `color.map.label`: `#c4d2de`
    ///
    /// map labels
    pub const MAP_LABEL: Rgba = Rgba::rgb(196, 210, 222);

    /// `color.map.line`: `#6f8aa3`
    ///
    /// canvas grid and outlines
    pub const MAP_LINE: Rgba = Rgba::rgb(111, 138, 163);

    /// `color.status.error`: `#ff9db0`
    pub const STATUS_ERROR: Rgba = Rgba::rgb(255, 157, 176);

    /// `color.status.ok`: `#8fe0a8`
    pub const STATUS_OK: Rgba = Rgba::rgb(143, 224, 168);

    /// `color.status.warn`: `#f0c674`
    pub const STATUS_WARN: Rgba = Rgba::rgb(240, 198, 116);

    /// `color.surface.button-float`: `rgba(12,15,22,.85)`
    ///
    /// round map buttons
    pub const SURFACE_BUTTON_FLOAT: Rgba = Rgba::new(12, 15, 22, 217);

    /// `color.surface.fill-1`: `rgba(255,255,255,.02)`
    pub const SURFACE_FILL_1: Rgba = Rgba::new(255, 255, 255, 5);

    /// `color.surface.fill-2`: `rgba(255,255,255,.025)`
    ///
    /// stat tiles, quip, kingdom cards, Gear --card
    pub const SURFACE_FILL_2: Rgba = Rgba::new(255, 255, 255, 6);

    /// `color.surface.fill-3`: `rgba(255,255,255,.035)`
    ///
    /// inputs, help note
    pub const SURFACE_FILL_3: Rgba = Rgba::new(255, 255, 255, 9);

    /// `color.surface.fill-4`: `rgba(255,255,255,.04)`
    ///
    /// Gear --card-2
    pub const SURFACE_FILL_4: Rgba = Rgba::new(255, 255, 255, 10);

    /// `color.surface.fill-focus`: `rgba(255,255,255,.06)`
    ///
    /// focused input; empty progress track
    pub const SURFACE_FILL_FOCUS: Rgba = Rgba::new(255, 255, 255, 15);

    /// `color.surface.glass`: `rgba(12,15,22,.88)`
    ///
    /// --glass; the Player ID gate
    pub const SURFACE_GLASS: Rgba = Rgba::new(12, 15, 22, 224);

    /// `color.surface.panel`: `rgba(12,15,22,.92)`
    ///
    /// panel body, with 14px backdrop blur
    pub const SURFACE_PANEL: Rgba = Rgba::new(12, 15, 22, 235);

    /// `color.surface.scrim`: `rgba(4,6,10,.55)`
    ///
    /// behind the pop-up sheet
    pub const SURFACE_SCRIM: Rgba = Rgba::new(4, 6, 10, 140);

    /// `color.surface.vignette`: `rgba(4,5,8,.8)`
    ///
    /// edge of the screen vignette
    pub const SURFACE_VIGNETTE: Rgba = Rgba::new(4, 5, 8, 204);

    /// `color.text.faint`: `#76869a`
    ///
    /// --faint; labels, captions
    pub const TEXT_FAINT: Rgba = Rgba::rgb(118, 134, 154);

    /// `color.text.ink`: `#effcff`
    ///
    /// --ink; body text and big numbers
    pub const TEXT_INK: Rgba = Rgba::rgb(239, 252, 255);

    /// `color.text.muted`: `#a3b3c2`
    ///
    /// --muted; secondary copy
    pub const TEXT_MUTED: Rgba = Rgba::rgb(163, 179, 194);

    /// `color.text.on-accent`: `#0b0f14`
    ///
    /// text on the light primary button
    pub const TEXT_ON_ACCENT: Rgba = Rgba::rgb(11, 15, 20);

    /// `color.text.strong`: `#e6eef5`
    ///
    /// --neon-hi; headings, values, primary button fill
    pub const TEXT_STRONG: Rgba = Rgba::rgb(230, 238, 245);

    /// `color.text.title-dim`: `#7d8ea0`
    ///
    /// the dimmed word in the hero title (Kingshot)
    pub const TEXT_TITLE_DIM: Rgba = Rgba::rgb(125, 142, 160);

    /// `color.tier.blue`: `#4f8fd9`
    pub const TIER_BLUE: Rgba = Rgba::rgb(79, 143, 217);

    /// `color.tier.gold`: `#d9c08a`
    pub const TIER_GOLD: Rgba = Rgba::rgb(217, 192, 138);

    /// `color.tier.green`: `#4fae72`
    pub const TIER_GREEN: Rgba = Rgba::rgb(79, 174, 114);

    /// `color.tier.none`: `#3a4448`
    pub const TIER_NONE: Rgba = Rgba::rgb(58, 68, 72);

    /// `color.tier.purple`: `#9a6fe0`
    pub const TIER_PURPLE: Rgba = Rgba::rgb(154, 111, 224);

    /// `color.tier.purple-t1`: `#b98af0`
    pub const TIER_PURPLE_T1: Rgba = Rgba::rgb(185, 138, 240);

    /// `color.tier.red`: `#e0654c`
    pub const TIER_RED: Rgba = Rgba::rgb(224, 101, 76);
}

pub mod font {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `font.letterSpacing.chip`: `0.08em`
    pub const LETTER_SPACING_CHIP_EM: f32 = 0.08;

    /// `font.letterSpacing.display`: `-0.035em`
    pub const LETTER_SPACING_DISPLAY_EM: f32 = -0.035;

    /// `font.letterSpacing.eyebrow`: `0.24em`
    pub const LETTER_SPACING_EYEBROW_EM: f32 = 0.24;

    /// `font.letterSpacing.heading`: `-0.01em`
    pub const LETTER_SPACING_HEADING_EM: f32 = -0.01;

    /// `font.letterSpacing.label`: `0.22em`
    pub const LETTER_SPACING_LABEL_EM: f32 = 0.22;

    /// `font.letterSpacing.number`: `-0.04em`
    pub const LETTER_SPACING_NUMBER_EM: f32 = -0.04;

    /// `font.letterSpacing.tight`: `-0.02em`
    pub const LETTER_SPACING_TIGHT_EM: f32 = -0.02;

    /// `font.size.ask`: `15px`
    ///
    /// question-style field label
    pub const SIZE_ASK: f32 = 15.0;

    /// `font.size.body`: `14.5px`
    ///
    /// quip; line-height 1.5
    pub const SIZE_BODY: f32 = 14.5;

    /// `font.size.button`: `14px`
    pub const SIZE_BUTTON: f32 = 14.0;

    /// `font.size.caption`: `13px`
    pub const SIZE_CAPTION: f32 = 13.0;

    /// `font.size.chip`: `10.5px`
    ///
    /// uppercase, .08em
    pub const SIZE_CHIP: f32 = 10.5;

    /// `font.size.display`: `clamp(36px,7vw,84px)`
    ///
    /// h1; line-height 1, letter-spacing -.035em, 700
    pub const SIZE_DISPLAY: f32 = 84.0;

    /// `font.size.egg`: `26px`
    pub const SIZE_EGG: f32 = 26.0;

    /// `font.size.eyebrow`: `12px`
    ///
    /// uppercase, letter-spacing .24em, 600, accent colour
    pub const SIZE_EYEBROW: f32 = 12.0;

    /// `font.size.heading-2`: `clamp(19px,2.6vw,22px)`
    ///
    /// panel title; -.01em, 600
    pub const SIZE_HEADING_2: f32 = 22.0;

    /// `font.size.heading-3`: `17px`
    pub const SIZE_HEADING_3: f32 = 17.0;

    /// `font.size.input`: `16px`
    ///
    /// inputs and selects (16px stops iOS zoom)
    pub const SIZE_INPUT: f32 = 16.0;

    /// `font.size.kingdom`: `22px`
    pub const SIZE_KINGDOM: f32 = 22.0;

    /// `font.size.label`: `10px`
    ///
    /// uppercase, letter-spacing .2-.22em, faint colour
    pub const SIZE_LABEL: f32 = 10.0;

    /// `font.size.lead`: `clamp(14px,2vw,16px)`
    ///
    /// hero tagline; line-height 1.7
    pub const SIZE_LEAD: f32 = 16.0;

    /// `font.size.meta`: `12.5px`
    pub const SIZE_META: f32 = 12.5;

    /// `font.size.micro`: `11px`
    pub const SIZE_MICRO: f32 = 11.0;

    /// `font.size.name`: `18px`
    pub const SIZE_NAME: f32 = 18.0;

    /// `font.size.result`: `clamp(60px,12vw,96px)`
    ///
    /// the big answer number; line-height .9, -.04em, 700, tabular
    pub const SIZE_RESULT: f32 = 96.0;

    /// `font.size.small`: `13.5px`
    pub const SIZE_SMALL: f32 = 13.5;

    /// `font.size.stat`: `clamp(24px,4vw,30px)`
    ///
    /// stat tile value; -.02em, 700, tabular
    pub const SIZE_STAT: f32 = 30.0;

    /// `font.weight.bold`: `700`
    pub const WEIGHT_BOLD: u16 = 700;

    /// `font.weight.medium`: `500`
    pub const WEIGHT_MEDIUM: u16 = 500;

    /// `font.weight.regular`: `400`
    pub const WEIGHT_REGULAR: u16 = 400;

    /// `font.weight.semibold`: `600`
    pub const WEIGHT_SEMIBOLD: u16 = 600;
}

pub mod motion {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `motion.duration.bar-fill`: `1600ms`
    pub const DURATION_BAR_FILL: Duration = ms(1600);

    /// `motion.duration.focus`: `250ms`
    pub const DURATION_FOCUS: Duration = ms(250);

    /// `motion.duration.haze`: `30s`
    ///
    /// background haze rotation, alternate
    pub const DURATION_HAZE: Duration = ms(30000);

    /// `motion.duration.hover`: `200ms`
    pub const DURATION_HOVER: Duration = ms(200);

    /// `motion.duration.letter`: `900ms`
    ///
    /// hero title letters, staggered 30ms
    pub const DURATION_LETTER: Duration = ms(900);

    /// `motion.duration.pop`: `600ms`
    pub const DURATION_POP: Duration = ms(600);

    /// `motion.duration.result-row`: `800ms`
    ///
    /// each result row, staggered 120ms
    pub const DURATION_RESULT_ROW: Duration = ms(800);

    /// `motion.duration.reveal`: `1000ms`
    pub const DURATION_REVEAL: Duration = ms(1000);

    /// `motion.duration.rise`: `1200ms`
    pub const DURATION_RISE: Duration = ms(1200);

    /// `motion.duration.shake`: `500ms`
    pub const DURATION_SHAKE: Duration = ms(500);

    /// `motion.duration.sheet`: `300ms`
    pub const DURATION_SHEET: Duration = ms(300);

    /// `motion.easing.bounce`: `[0.3,1.5,0.5,1]`
    ///
    /// ticket drop
    pub const EASING_BOUNCE: CubicBezier = CubicBezier(0.3, 1.5, 0.5, 1.0);

    /// `motion.easing.hop`: `[0.5,0,0.5,1]`
    ///
    /// loading diamonds
    pub const EASING_HOP: CubicBezier = CubicBezier(0.5, 0.0, 0.5, 1.0);

    /// `motion.easing.out`: `[0.2,0.8,0.2,1]`
    ///
    /// the house curve: rises, reveals, bars, sheet
    pub const EASING_OUT: CubicBezier = CubicBezier(0.2, 0.8, 0.2, 1.0);

    /// `motion.easing.spring`: `[0.3,1.6,0.5,1]`
    ///
    /// number pop
    pub const EASING_SPRING: CubicBezier = CubicBezier(0.3, 1.6, 0.5, 1.0);
}

pub mod radius {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `radius.gate`: `18px`
    ///
    /// Player ID gate; Gear stage card
    pub const GATE: f32 = 18.0;

    /// `radius.lg`: `10px`
    ///
    /// icon buttons, Gear inputs
    pub const LG: f32 = 10.0;

    /// `radius.md`: `6px`
    ///
    /// calendar day
    pub const MD: f32 = 6.0;

    /// `radius.panel`: `14px`
    ///
    /// panels, stat tiles, kingdom cards, sheet
    pub const PANEL: f32 = 14.0;

    /// `radius.panel-inner`: `13px`
    pub const PANEL_INNER: f32 = 13.0;

    /// `radius.pill`: `99px`
    ///
    /// chips, progress bars, Gear buttons (999px)
    pub const PILL: f32 = 99.0;

    /// `radius.sm`: `5px`
    ///
    /// event bar
    pub const SM: f32 = 5.0;

    /// `radius.xl`: `12px`
    ///
    /// inputs, primary button, notes, errors
    pub const XL: f32 = 12.0;

    /// `radius.xs`: `3px`
    ///
    /// ticket
    pub const XS: f32 = 3.0;
}

pub mod shadow {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `shadow.focus`: `0 0 0 3px rgba(143,184,216,.18)`
    ///
    /// input focus ring, with border-color accent
    pub const FOCUS: Shadow = Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 3.0, color: Rgba::new(143, 184, 216, 46) };

    /// `shadow.panel`: `0 24px 60px -24px rgba(0,0,0,.7)`
    pub const PANEL: Shadow = Shadow { x: 0.0, y: 24.0, blur: 60.0, spread: -24.0, color: Rgba::new(0, 0, 0, 179) };

    /// `shadow.sheet`: `0 30px 80px -20px rgba(0,0,0,.8)`
    pub const SHEET: Shadow = Shadow { x: 0.0, y: 30.0, blur: 80.0, spread: -20.0, color: Rgba::new(0, 0, 0, 204) };

    /// `shadow.success-glow`: `0 0 30px -12px rgba(143,224,168,.7)`
    ///
    /// kingdom card you can afford
    pub const SUCCESS_GLOW: Shadow = Shadow { x: 0.0, y: 0.0, blur: 30.0, spread: -12.0, color: Rgba::new(143, 224, 168, 179) };
}

pub mod size {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `size.avatar`: `56px`
    pub const AVATAR: f32 = 56.0;

    /// `size.button-height`: `50px`
    pub const BUTTON_HEIGHT: f32 = 50.0;

    /// `size.content-max`: `1180px`
    pub const CONTENT_MAX: f32 = 1180.0;

    /// `size.egg-card`: `380px`
    pub const EGG_CARD: f32 = 380.0;

    /// `size.gate`: `460px`
    pub const GATE: f32 = 460.0;

    /// `size.help-button`: `24px`
    pub const HELP_BUTTON: f32 = 24.0;

    /// `size.icon-button`: `40px`
    ///
    /// 34px inside the sheet header, 32px for card close
    pub const ICON_BUTTON: f32 = 40.0;

    /// `size.panel`: `560px`
    pub const PANEL: f32 = 560.0;

    /// `size.panel-wide`: `640px`
    pub const PANEL_WIDE: f32 = 640.0;

    /// `size.pin`: `8px`
    pub const PIN: f32 = 8.0;

    /// `size.progress`: `10px`
    pub const PROGRESS: f32 = 10.0;

    /// `size.sheet`: `1040px x 760px`
    pub const SHEET: [f32; 2] = [1040.0, 760.0];

    /// `size.soon-card`: `400px`
    pub const SOON_CARD: f32 = 400.0;
}

pub mod space {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `space.1`: `4px`
    pub const STEP_1: f32 = 4.0;

    /// `space.1-5`: `6px`
    pub const STEP_1_5: f32 = 6.0;

    /// `space.2`: `8px`
    pub const STEP_2: f32 = 8.0;

    /// `space.2-5`: `10px`
    pub const STEP_2_5: f32 = 10.0;

    /// `space.3`: `12px`
    pub const STEP_3: f32 = 12.0;

    /// `space.3-5`: `14px`
    pub const STEP_3_5: f32 = 14.0;

    /// `space.4`: `16px`
    ///
    /// phone side gutter; tile padding
    pub const STEP_4: f32 = 16.0;

    /// `space.4-5`: `18px`
    pub const STEP_4_5: f32 = 18.0;

    /// `space.5`: `20px`
    pub const STEP_5: f32 = 20.0;

    /// `space.5-5`: `22px`
    pub const STEP_5_5: f32 = 22.0;

    /// `space.6`: `26px`
    ///
    /// panel padding (desktop); phone is 20px 16px
    pub const STEP_6: f32 = 26.0;

    /// `space.8`: `30px`
    pub const STEP_8: f32 = 30.0;

    /// `space.section`: `96px`
    ///
    /// vertical padding of each stop (desktop)
    pub const SECTION: f32 = 96.0;
}

pub mod z {
    #[allow(unused_imports)]
    use crate::{CubicBezier, Rgba, Shadow, ms};
    #[allow(unused_imports)]
    use std::time::Duration;

    /// `z.content`: `0`
    pub const CONTENT: i32 = 0;

    /// `z.egg-card`: `2`
    pub const EGG_CARD: i32 = 2;

    /// `z.haze`: `-3`
    pub const HAZE: i32 = -3;

    /// `z.map`: `-2`
    pub const MAP: i32 = -2;

    /// `z.sheet`: `60`
    pub const SHEET: i32 = 60;

    /// `z.sky`: `-4`
    pub const SKY: i32 = -4;

    /// `z.veil`: `-1`
    pub const VEIL: i32 = -1;
}
