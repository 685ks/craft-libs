//! Generates `src/generated.rs` from `tokens.json` and fails when the committed file differs.
//!
//! Regenerate after editing `tokens.json` with:
//! `CRAFT_TOKENS_REGENERATE=1 cargo test -p craft-tokens --test generated_is_current`

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

use serde_json::Value;

#[test]
fn generated_rs_matches_tokens_json() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let json: Value = serde_json::from_str(&std::fs::read_to_string(root.join("tokens.json")).unwrap()).unwrap();
    let generated = generate(&json);
    let path = root.join("src/generated.rs");
    if std::env::var_os("CRAFT_TOKENS_REGENERATE").is_some() {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "src/generated.rs is out of date with tokens.json; run `CRAFT_TOKENS_REGENERATE=1 cargo test -p craft-tokens --test generated_is_current`"
    );
}

struct Token {
    name: String,
    ty: String,
    value: Value,
    description: Option<String>,
}

fn generate(json: &Value) -> String {
    let mut groups: BTreeMap<String, Vec<Token>> = BTreeMap::new();
    let Value::Object(top) = json else { panic!("tokens.json must be an object") };
    for (group, node) in top.iter().filter(|(k, _)| !k.starts_with('$')) {
        let mut tokens = Vec::new();
        collect(node, &mut Vec::new(), None, &mut tokens);
        groups.insert(group.clone(), tokens);
    }

    let mut out = String::from("// @generated from tokens.json by tests/generated_is_current.rs. Do not edit by hand.\n");
    for (group, tokens) in &groups {
        let items: Vec<String> = tokens.iter().filter_map(|t| item(group, t)).collect();
        if items.is_empty() {
            continue;
        }
        writeln!(out, "\npub mod {} {{", ident(group).to_lowercase()).unwrap();
        writeln!(out, "    #[allow(unused_imports)]\n    use crate::{{CubicBezier, Rgba, Shadow, ms}};\n    #[allow(unused_imports)]\n    use std::time::Duration;").unwrap();
        for item in items {
            out.push('\n');
            out.push_str(&item);
        }
        out.push_str("}\n");
    }
    out
}

fn collect(node: &Value, path: &mut Vec<String>, inherited: Option<&str>, out: &mut Vec<Token>) {
    let Value::Object(map) = node else { return };
    let ty = map.get("$type").and_then(Value::as_str).or(inherited);
    if let Some(value) = map.get("$value") {
        out.push(Token {
            name: path.join("_"),
            ty: ty.unwrap_or("").to_string(),
            value: value.clone(),
            description: map.get("$description").and_then(Value::as_str).map(str::to_string),
        });
        return;
    }
    for (key, child) in map.iter().filter(|(k, _)| !k.starts_with('$')) {
        path.push(key.clone());
        collect(child, path, ty, out);
        path.pop();
    }
}

fn ident(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) { format!("STEP_{s}") } else { s }
}

/// Turns camelCase keys such as `letterSpacing` into `LETTER_SPACING`.
fn const_name(name: &str) -> String {
    let mut s = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            s.push('_');
        }
        s.push(c);
    }
    ident(&s)
}

fn item(group: &str, t: &Token) -> Option<String> {
    let name = const_name(&t.name);
    let (ty, expr) = match (t.ty.as_str(), &t.value) {
        ("color", Value::String(s)) => ("Rgba", color(s)?),
        ("fontWeight", Value::Number(n)) => ("u16", n.to_string()),
        ("number", Value::Number(n)) => ("i32", n.to_string()),
        ("cubicBezier", Value::Array(v)) => {
            let p: Vec<String> = v.iter().map(|x| float(x.as_f64().unwrap_or(0.0))).collect();
            ("CubicBezier", format!("CubicBezier({})", p.join(", ")))
        }
        ("duration", Value::String(s)) => ("Duration", format!("ms({})", duration_ms(s)?)),
        ("shadow", Value::String(s)) => ("Shadow", shadow(s)?),
        ("dimension", Value::String(s)) => {
            if let Some((w, h)) = s.split_once(" x ") {
                ("[f32; 2]", format!("[{}, {}]", float(px(w)?), float(px(h)?)))
            } else if let Some(em) = s.strip_suffix("em") {
                return Some(doc_and_const(group, t, &format!("{name}_EM"), "f32", &float(em.parse().ok()?)));
            } else {
                ("f32", float(px(s)?))
            }
        }
        _ => return None,
    };
    Some(doc_and_const(group, t, &name, ty, &expr))
}

fn doc_and_const(group: &str, t: &Token, name: &str, ty: &str, expr: &str) -> String {
    let mut s = String::new();
    let source = format!("`{group}.{}`: `{}`", t.name.replace('_', "."), t.value.to_string().trim_matches('"'));
    writeln!(s, "    /// {source}").unwrap();
    if let Some(d) = &t.description {
        writeln!(s, "    ///\n    /// {}", d.replace('\n', " ")).unwrap();
    }
    writeln!(s, "    pub const {name}: {ty} = {expr};").unwrap();
    s
}

fn float(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') { s } else { format!("{s}.0") }
}

/// A pixel value, or the desktop (largest) end of a CSS `clamp(min, preferred, max)`.
fn px(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("clamp(").and_then(|r| r.strip_suffix(')')) {
        return px(inner.rsplit(',').next()?);
    }
    s.strip_suffix("px")?.trim().parse().ok()
}

fn duration_ms(s: &str) -> Option<u64> {
    if let Some(v) = s.strip_suffix("ms") {
        v.parse::<f64>().ok().map(|v| v.round() as u64)
    } else {
        s.strip_suffix('s')?.parse::<f64>().ok().map(|v| (v * 1000.0).round() as u64)
    }
}

fn color(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        return match hex.len() {
            6 => Some(format!("Rgba::rgb({}, {}, {})", byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(format!("Rgba::new({}, {}, {}, {})", byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        };
    }
    let inner = s.strip_prefix("rgba(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    let [r, g, b, a] = parts.as_slice() else { return None };
    let alpha = (a.parse::<f64>().ok()? * 255.0).round() as u8;
    Some(format!("Rgba::new({r}, {g}, {b}, {alpha})"))
}

/// `x y blur spread rgba(...)` with px units.
fn shadow(s: &str) -> Option<String> {
    let (lengths, rest) = s.split_once("rgba(")?;
    let nums: Vec<f64> = lengths.split_whitespace().map(|v| v.trim_end_matches("px").parse().ok()).collect::<Option<_>>()?;
    let [x, y, blur, spread] = nums.as_slice() else { return None };
    let c = color(&format!("rgba({rest}"))?;
    Some(format!("Shadow {{ x: {}, y: {}, blur: {}, spread: {}, color: {c} }}", float(*x), float(*y), float(*blur), float(*spread)))
}
