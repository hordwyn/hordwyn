//! WCAG 2.x contrast of theme colours (design.md 2.7: an operator who sets `[theme] primary`
//! must keep `primary-foreground` at >= 4.5:1).
//!
//! Relative luminance and contrast ratio as defined in WCAG 2.2, "relative luminance" and
//! "contrast ratio" (<https://www.w3.org/TR/WCAG22/#dfn-relative-luminance>). `oklch()` is
//! converted with the matrices of CSS Color 4, section 10.8
//! (<https://www.w3.org/TR/css-color-4/#color-conversion-code>), clipped to sRGB.

/// Minimum ratio for normal text (WCAG 1.4.3, AA).
pub const AA_TEXT: f64 = 4.5;

/// An sRGB colour, 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// WCAG relative luminance, 0 (black) to 1 (white).
    #[must_use]
    pub fn luminance(self) -> f64 {
        let lin = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.0) + 0.7152 * lin(self.1) + 0.0722 * lin(self.2)
    }
}

/// Contrast ratio of two colours, 1 to 21, independent of the order.
#[must_use]
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Parse the colour forms a brand file uses: `#rgb`, `#rrggbb`, `rgb(r g b)` / `rgb(r, g, b)`,
/// `oklch(L C H)` (L as a number or percentage), `white`, `black`. Anything else (`var()`,
/// other functions, alpha) gives `None`: it cannot be checked.
#[must_use]
pub fn parse_color(v: &str) -> Option<Rgb> {
    let v = v.trim().to_ascii_lowercase();
    match v.as_str() {
        "white" => return Some(Rgb(255, 255, 255)),
        "black" => return Some(Rgb(0, 0, 0)),
        _ => {}
    }
    if let Some(hex) = v.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(args) = function_args(&v, "rgb") {
        let nums = numbers(args)?;
        let [red, green, blue] = nums.as_slice() else {
            return None;
        };
        return Some(Rgb(
            channel(*red / 255.0)?,
            channel(*green / 255.0)?,
            channel(*blue / 255.0)?,
        ));
    }
    if let Some(args) = function_args(&v, "oklch") {
        return parse_oklch(args);
    }
    None
}

fn function_args<'a>(v: &'a str, name: &str) -> Option<&'a str> {
    v.strip_prefix(name)?
        .trim_start()
        .strip_prefix('(')?
        .strip_suffix(')')
}

/// Space or comma separated numbers; a trailing `%` divides by 100.
fn numbers(args: &str) -> Option<Vec<f64>> {
    args.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| match s.strip_suffix('%') {
            Some(p) => p.parse::<f64>().ok().map(|x| x / 100.0),
            None => s.parse::<f64>().ok(),
        })
        .collect()
}

fn parse_hex(hex: &str) -> Option<Rgb> {
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |s: &str| u8::from_str_radix(s, 16).ok();
    match hex.len() {
        3 => {
            let d: Vec<String> = hex.chars().map(|c| format!("{c}{c}")).collect();
            Some(Rgb(byte(&d[0])?, byte(&d[1])?, byte(&d[2])?))
        }
        6 => Some(Rgb(byte(&hex[0..2])?, byte(&hex[2..4])?, byte(&hex[4..6])?)),
        _ => None,
    }
}

/// 0..1 (clipped) -> 0..255.
fn channel(x: f64) -> Option<u8> {
    if !x.is_finite() {
        return None;
    }
    // clamped to 0..=255 first, so the cast cannot truncate or wrap
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((x.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn parse_oklch(args: &str) -> Option<Rgb> {
    let nums = numbers(args)?;
    let [light, chroma, hue] = nums.as_slice() else {
        return None;
    };
    let (light, lab_a, lab_b) = (
        *light,
        chroma * hue.to_radians().cos(),
        chroma * hue.to_radians().sin(),
    );
    // OKLab -> LMS (cube roots)
    let l_ = light + 0.396_337_777_4 * lab_a + 0.215_803_757_3 * lab_b;
    let m_ = light - 0.105_561_345_8 * lab_a - 0.063_854_172_8 * lab_b;
    let s_ = light - 0.089_484_177_5 * lab_a - 1.291_485_548 * lab_b;
    let (lc, mc, sc) = (l_.powi(3), m_.powi(3), s_.powi(3));
    // LMS -> linear sRGB
    let red = 4.076_741_662_1 * lc - 3.307_711_591_3 * mc + 0.230_969_929_2 * sc;
    let green = -1.268_438_004_6 * lc + 2.609_757_401_1 * mc - 0.341_319_396_5 * sc;
    let blue = -0.004_196_086_3 * lc - 0.703_418_614_7 * mc + 1.707_614_701 * sc;
    let gamma = |lin: f64| {
        if lin <= 0.003_130_8 {
            12.92 * lin
        } else {
            1.055 * lin.powf(1.0 / 2.4) - 0.055
        }
    };
    Some(Rgb(
        channel(gamma(red))?,
        channel(gamma(green))?,
        channel(gamma(blue))?,
    ))
}

/// One theme whose accent pair is below [`AA_TEXT`] or could not be checked.
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastFinding {
    /// `light` or `dark`.
    pub theme: &'static str,
    pub primary: String,
    pub foreground: String,
    /// `None` when a value is not a colour this module can read.
    pub ratio: Option<f64>,
}

/// Defaults of `src/web/style/input.css` (design.md 2.7): zinc-900 / zinc-50 in light,
/// zinc-50 / zinc-900 in dark.
const DEFAULTS: [(&str, &str, &str); 2] = [
    ("light", "#18181b", "#fafafa"),
    ("dark", "#fafafa", "#18181b"),
];

/// Check `primary` against `primary-foreground` per theme; a theme the brand does not touch
/// keeps the stylesheet defaults, which pass.
pub(crate) fn findings(
    light: &std::collections::BTreeMap<String, String>,
    dark: &std::collections::BTreeMap<String, String>,
) -> Vec<ContrastFinding> {
    let get = |m: &std::collections::BTreeMap<String, String>, key: &str| {
        m.iter()
            .find(|(k, _)| k.trim_start_matches("--") == key)
            .map(|(_, v)| v.clone())
    };
    let mut out = Vec::new();
    for ((theme, def_p, def_f), vars) in DEFAULTS.into_iter().zip([light, dark]) {
        let (p, f) = (get(vars, "primary"), get(vars, "primary-foreground"));
        if p.is_none() && f.is_none() {
            continue;
        }
        let primary = p.unwrap_or_else(|| def_p.into());
        let foreground = f.unwrap_or_else(|| def_f.into());
        let ratio = parse_color(&primary)
            .zip(parse_color(&foreground))
            .map(|(a, b)| contrast_ratio(a, b));
        if ratio.is_none_or(|r| r < AA_TEXT) {
            out.push(ContrastFinding {
                theme,
                primary,
                foreground,
                ratio,
            });
        }
    }
    out
}
