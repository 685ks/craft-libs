# craft-tokens

The house design tokens for the Craft apps: the tools-site design system as Rust constants.

`tokens.json` (W3C design-token format) is the source of truth, and `tokens.css` is the same set as a CSS `:root` block for web builds. `src/generated.rs` is generated from the JSON; edit the JSON, then run:

```sh
CRAFT_TOKENS_REGENERATE=1 cargo test -p craft-tokens --test generated_is_current
```

The same test fails in CI when the two drift apart.

Constants are grouped by the JSON's top-level groups (`color`, `font`, `space`, `size`, `radius`, `shadow`, `motion`, ...). Web-only values are reduced: a CSS `clamp()` becomes its desktop (largest) end, and values in `vh` or `%` are left out. Translucent colours keep their alpha; use `Rgba::over` to flatten one onto an opaque surface.

```rust
use craft_tokens::{color, motion, radius};

let panel = color::SURFACE_PANEL.over(color::BG_BASE); // opaque #0c0f15
let fade = motion::EASING_OUT.ease(0.5);
let r = radius::PANEL; // 14.0
```

With the `egui` feature, `Rgba` converts into `egui::Color32` and `Shadow` into `egui::Shadow`. `craft-ui`'s `house` module builds the full egui theme and the shared components from these values.
