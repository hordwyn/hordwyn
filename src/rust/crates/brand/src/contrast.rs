//! WCAG 2.x contrast of theme colours (design.md 2.7: an operator who sets `[theme] primary`
//! must keep `primary-foreground` at >= 4.5:1).
//!
//! Relative luminance and contrast ratio as defined in WCAG 2.2, "relative luminance" and
//! "contrast ratio" (<https://www.w3.org/TR/WCAG22/#dfn-relative-luminance>). `oklch()` is
//! converted with the matrices of CSS Color 4, section 10.8
//! (<https://www.w3.org/TR/css-color-4/#color-conversion-code>), clipped to sRGB.

/// Minimum ratio for normal text (WCAG 1.4.3, AA).
pub const AA_TEXT: f64 = 4.5;

/// Minimum ratio for graphics and parts of controls (WCAG 1.4.11, AA).
pub const AA_NON_TEXT: f64 = 3.0;

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

/// The accent pair of one theme with its ratio (the swatches of the admin Branding page).
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

/// The accent pair of both themes with its ratio, defaults filled in.
pub(crate) fn pairs(
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
        let primary = p.unwrap_or_else(|| def_p.into());
        let foreground = f.unwrap_or_else(|| def_f.into());
        let ratio = parse_color(&primary)
            .zip(parse_color(&foreground))
            .map(|(a, b)| contrast_ratio(a, b));
        out.push(ContrastFinding {
            theme,
            primary,
            foreground,
            ratio,
        });
    }
    out
}

/// One checked colour pair of a theme: a token used as text, icon or bar on a surface token.
#[derive(Debug, Clone, PartialEq)]
pub struct PairCheck {
    /// `light` or `dark`.
    pub theme: &'static str,
    /// Token of the text, icon or bar (`group-watch`). `<token>/<percent>` is the token at
    /// that opacity over the surface (`primary-foreground/75`: the count in a pressed chip).
    pub foreground_token: String,
    /// Token of the surface under it (`group-watch-soft`).
    pub background_token: String,
    /// The values the tokens resolve to (the brand's, else the stylesheet's); for a
    /// foreground with an opacity the blended colour as hex.
    pub foreground: String,
    pub background: String,
    /// `None` when a value is not a colour this module can read.
    pub ratio: Option<f64>,
    /// [`AA_TEXT`] or [`AA_NON_TEXT`].
    pub required: f64,
}

impl PairCheck {
    /// Below its threshold, or not checkable.
    #[must_use]
    pub fn fails(&self) -> bool {
        self.ratio.is_none_or(|r| r < self.required)
    }
}

/// The activity groups that have a tint (`group-<id>` and `group-<id>-soft`).
pub const TINT_GROUPS: [&str; 4] = ["watch", "read", "listen", "play"];

/// What a token is when the brand does not set it: the values of `src/web/style/input.css`
/// (Tailwind 4.1 zinc: 50 `#fafafa`, 100 `#f4f4f5`, 400 `#9f9fa9`, 600 `#52525c`, 800
/// `#27272a`, 900 `#18181b`, 950 `#09090b`; sky-50 `#f0f9ff`), or another token
/// (`var(--muted)` there). The test `the_fallbacks_are_those_of_the_stylesheet` reads the
/// stylesheet and fails when the two drift apart (review OT2D-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fallback {
    /// Light and dark value.
    Value(&'static str, &'static str),
    Token(&'static str),
}

fn fallback(token: &str) -> Option<Fallback> {
    use Fallback::{Token, Value};
    Some(match token {
        "background" => Value("#ffffff", "#09090b"),
        "foreground" => Value("#09090b", "#fafafa"),
        "card" => Value("#ffffff", "#18181b"),
        "muted" | "secondary" => Value("#f4f4f5", "#27272a"),
        "muted-foreground" => Value("#52525c", "#9f9fa9"),
        "info-soft" => Value("#f0f9ff", "#052235"),
        "primary" => Value(DEFAULTS[0].1, DEFAULTS[1].1),
        "primary-foreground" => Value(DEFAULTS[0].2, DEFAULTS[1].2),
        "primary-soft" => Token("secondary"),
        t if t.starts_with("group-") && t.ends_with("-soft") => Token("muted"),
        t if t.starts_with("group-") => Token("muted-foreground"),
        _ => return None,
    })
}

/// The value of `token` in one theme: the brand's, else the stylesheet fallback.
fn resolve(vars: &std::collections::BTreeMap<String, String>, dark: bool, token: &str) -> String {
    if let Some((_, v)) = vars
        .iter()
        .find(|(k, _)| k.trim_start_matches("--") == token)
    {
        return v.clone();
    }
    match fallback(token) {
        Some(Fallback::Value(light, dark_value)) => {
            if dark { dark_value } else { light }.to_string()
        }
        Some(Fallback::Token(other)) => resolve(vars, dark, other),
        None => String::new(),
    }
}

/// `fg` at `alpha` over `bg`, blended in sRGB and rounded to 8 bits as a browser paints it.
fn over(fg: Rgb, bg: Rgb, alpha: f64) -> Rgb {
    // 0..=255 by construction
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mix = |a: u8, b: u8| (f64::from(a) * alpha + f64::from(b) * (1.0 - alpha)).round() as u8;
    Rgb(mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2))
}

/// The value of a foreground token on `background`. `<token>/<percent>` (the count of a
/// pressed chip is `primary-foreground` at 75 %) is blended over the surface and given as
/// hex; when one of the two cannot be read, the token's own value is returned, so the pair
/// is reported as not checkable.
fn foreground_value(
    vars: &std::collections::BTreeMap<String, String>,
    dark: bool,
    token: &str,
    background: &str,
) -> String {
    let Some((base, percent)) = token.split_once('/') else {
        return resolve(vars, dark, token);
    };
    let value = resolve(vars, dark, base);
    let alpha = percent.parse::<f64>().map_or(1.0, |p| p / 100.0);
    match (parse_color(&value), parse_color(background)) {
        (Some(fg), Some(bg)) => {
            let Rgb(r, g, b) = over(fg, bg, alpha);
            format!("#{r:02x}{g:02x}{b:02x}")
        }
        // not a colour this module reads: `checks` finds no ratio either
        _ => format!("{value} at {percent} %"),
    }
}

/// The count of a pressed chip: `primary-foreground` at 75 % opacity on `primary`
/// (`ui/src/chip.rs`, review OT2D-7). It needs about 7.2:1 for the plain pair.
pub const PRESSED_COUNT: &str = "primary-foreground/75";

/// The pairs the stylesheet and the components use (foreground token, surface token,
/// threshold): the accent as button, link, bar and "selected" surface, and per group the
/// tint as icon, short label and bar plus the text on its soft surface.
fn checked_pairs() -> Vec<(String, String, f64)> {
    let mut out: Vec<(String, String, f64)> = [
        ("primary-foreground", "primary", AA_TEXT),
        (PRESSED_COUNT, "primary", AA_TEXT),
        ("primary", "background", AA_TEXT),
        ("primary", "card", AA_TEXT),
        ("primary", "muted", AA_NON_TEXT),
        ("primary", "primary-soft", AA_TEXT),
        ("foreground", "primary-soft", AA_TEXT),
        ("muted-foreground", "primary-soft", AA_TEXT),
    ]
    .into_iter()
    .map(|(fg, bg, min)| (fg.to_string(), bg.to_string(), min))
    .collect();
    for g in TINT_GROUPS {
        let (tint, soft) = (format!("group-{g}"), format!("group-{g}-soft"));
        out.push((tint.clone(), "background".into(), AA_TEXT));
        out.push((tint.clone(), "card".into(), AA_TEXT));
        out.push((tint.clone(), "muted".into(), AA_NON_TEXT));
        // the kind icon of a shared History row sits on the info surface
        out.push((tint.clone(), "info-soft".into(), AA_NON_TEXT));
        out.push((tint, soft.clone(), AA_TEXT));
        out.push(("foreground".into(), soft.clone(), AA_TEXT));
        out.push(("muted-foreground".into(), soft, AA_TEXT));
    }
    out
}

/// Every checked pair of both themes with its ratio (DEC-540).
pub(crate) fn checks(
    light: &std::collections::BTreeMap<String, String>,
    dark: &std::collections::BTreeMap<String, String>,
) -> Vec<PairCheck> {
    let pairs = checked_pairs();
    let mut out = Vec::with_capacity(pairs.len() * 2);
    for (theme, vars, is_dark) in [("light", light, false), ("dark", dark, true)] {
        for (fg, bg, required) in &pairs {
            let background = resolve(vars, is_dark, bg);
            let foreground = foreground_value(vars, is_dark, fg, &background);
            let ratio = parse_color(&foreground)
                .zip(parse_color(&background))
                .map(|(a, b)| contrast_ratio(a, b));
            out.push(PairCheck {
                theme,
                foreground_token: fg.clone(),
                background_token: bg.clone(),
                foreground,
                background,
                ratio,
                required: *required,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Fallback, PRESSED_COUNT, TINT_GROUPS, checked_pairs, fallback};

    /// Tailwind 4.1 values of the palette names the stylesheet's fallbacks use (design.md
    /// 2.7, from `tailwindcss@4.1.0/theme.css`).
    const PALETTE: [(&str, &str); 9] = [
        ("white", "#ffffff"),
        ("zinc-50", "#fafafa"),
        ("zinc-100", "#f4f4f5"),
        ("zinc-400", "#9f9fa9"),
        ("zinc-600", "#52525c"),
        ("zinc-800", "#27272a"),
        ("zinc-900", "#18181b"),
        ("zinc-950", "#09090b"),
        ("sky-50", "#f0f9ff"),
    ];

    /// The declarations of the first block that `selector` opens in the stylesheet.
    fn block(css: &str, selector: &str) -> Vec<(String, String)> {
        let start = css
            .find(&format!("\n{selector} {{"))
            .unwrap_or_else(|| panic!("no {selector} block"));
        let body = &css[start..];
        let body = &body[..body.find("\n}").expect("the block ends")];
        body.lines()
            .filter_map(|l| {
                l.trim()
                    .strip_prefix("--")?
                    .strip_suffix(';')?
                    .split_once(':')
            })
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect()
    }

    /// What the stylesheet declares for `token`: a value as hex, or another token.
    fn declared(block: &[(String, String)], token: &str) -> Result<String, String> {
        let value = &block
            .iter()
            .find(|(k, _)| k == token)
            .unwrap_or_else(|| panic!("--{token} is not declared"))
            .1;
        if let Some(name) = value
            .strip_prefix("var(--color-")
            .and_then(|v| v.strip_suffix(')'))
        {
            let hex = PALETTE
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or_else(|| panic!("--{token}: {name} is not in the test's palette table"));
            return Ok(hex.1.to_string());
        }
        match value
            .strip_prefix("var(--")
            .and_then(|v| v.strip_suffix(')'))
        {
            Some(other) => Err(other.to_string()),
            None => Ok(value.clone()),
        }
    }

    #[test]
    fn the_fallbacks_are_those_of_the_stylesheet() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../web/style/input.css");
        let css = std::fs::read_to_string(&path).unwrap();
        let (light, dark) = (block(&css, ":root"), block(&css, ".dark"));
        // every token a checked pair names (an opacity is not a token of its own)
        let mut tokens: Vec<String> = checked_pairs()
            .into_iter()
            .flat_map(|(fg, bg, _)| [fg, bg])
            .map(|t| t.split('/').next().unwrap_or_default().to_string())
            .collect();
        tokens.push("secondary".into());
        tokens.sort();
        tokens.dedup();
        assert_eq!(tokens.len(), 10 + 2 * TINT_GROUPS.len(), "{tokens:?}");
        for token in tokens {
            let expected = fallback(&token).unwrap_or_else(|| panic!("no fallback for {token}"));
            let found = match (declared(&light, &token), declared(&dark, &token)) {
                (Ok(l), Ok(d)) => format!("{l} / {d}"),
                (Err(l), Err(d)) if l == d => format!("var(--{l})"),
                other => panic!("--{token}: a value in one theme, a token in the other: {other:?}"),
            };
            let mirrored = match expected {
                Fallback::Value(l, d) => format!("{l} / {d}"),
                Fallback::Token(other) => format!("var(--{other})"),
            };
            assert_eq!(
                mirrored, found,
                "--{token}: contrast.rs (left) and input.css (right) differ"
            );
        }
    }

    #[test]
    fn the_pressed_chip_count_is_a_checked_pair() {
        assert!(
            checked_pairs()
                .iter()
                .any(|(fg, bg, min)| fg == PRESSED_COUNT && bg == "primary" && *min >= 4.5)
        );
        // the opacity the check assumes is the one the component uses
        let chip = include_str!("../../ui/src/chip.rs");
        assert!(
            chip.contains("\"font-normal opacity-75\""),
            "chip.rs changed"
        );
    }
}
