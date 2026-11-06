//! Storage budget configuration (D-8). Every key is optional; defaults match DESIGN.md.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A byte count written as `20GiB`, `512MiB`, `1.5GB` or a plain number of bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct ByteSize(pub u64);

const UNITS: [(&str, u64); 9] = [
    ("TiB", 1 << 40),
    ("GiB", 1 << 30),
    ("MiB", 1 << 20),
    ("KiB", 1 << 10),
    ("TB", 1_000_000_000_000),
    ("GB", 1_000_000_000),
    ("MB", 1_000_000),
    ("KB", 1_000),
    ("B", 1),
];

impl ByteSize {
    #[must_use]
    pub const fn gib(n: u64) -> Self {
        Self(n << 30)
    }

    #[must_use]
    pub const fn mib(n: u64) -> Self {
        Self(n << 20)
    }
}

impl FromStr for ByteSize {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (num, mult) = UNITS
            .iter()
            .find_map(|(u, m)| s.strip_suffix(u).map(|n| (n.trim(), *m)))
            .unwrap_or((s, 1));
        let value: f64 = num
            .parse()
            .map_err(|_| format!("invalid byte size `{s}`"))?;
        if !value.is_finite() || value < 0.0 {
            return Err(format!("invalid byte size `{s}`"));
        }
        // Sizes are far below 2^53, so the float round trip is exact enough.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        Ok(Self((value * mult as f64).round() as u64))
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (unit, mult) in &UNITS[..4] {
            if self.0 >= *mult && self.0.is_multiple_of(*mult) {
                return write!(f, "{}{unit}", self.0 / mult);
            }
        }
        write!(f, "{}B", self.0)
    }
}

impl Serialize for ByteSize {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Num(u64),
            Text(String),
        }
        match Raw::deserialize(d)? {
            Raw::Num(n) => Ok(Self(n)),
            Raw::Text(t) => t.parse().map_err(serde::de::Error::custom),
        }
    }
}

/// Free space that must stay available on the volume (fs backend only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinFree {
    Bytes(ByteSize),
    /// Fraction of the volume size, `0.0..=1.0` (written as `10%`).
    Fraction(f64),
}

impl MinFree {
    /// Required free bytes on a volume of `total` bytes.
    #[must_use]
    pub fn required(&self, total: u64) -> u64 {
        match *self {
            Self::Bytes(b) => b.0,
            // Volume sizes fit in f64 without meaningful loss.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            Self::Fraction(f) => (total as f64 * f).ceil() as u64,
        }
    }
}

impl FromStr for MinFree {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if let Some(p) = s.strip_suffix('%') {
            let v: f64 = p
                .trim()
                .parse()
                .map_err(|_| format!("invalid percentage `{s}`"))?;
            if !(0.0..=100.0).contains(&v) {
                return Err(format!("percentage out of range `{s}`"));
            }
            return Ok(Self::Fraction(v / 100.0));
        }
        s.parse().map(Self::Bytes)
    }
}

impl Serialize for MinFree {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Bytes(b) => b.serialize(s),
            Self::Fraction(f) => s.serialize_str(&format!("{}%", f * 100.0)),
        }
    }
}

impl<'de> Deserialize<'de> for MinFree {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Image variants (D-7). Widths are upper bounds; images are never upscaled.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    Thumb,
    Card,
    Detail,
    Full,
}

impl Variant {
    pub const ALL: [Self; 4] = [Self::Thumb, Self::Card, Self::Detail, Self::Full];

    /// Target width in pixels; `None` keeps the (capped) source width.
    #[must_use]
    pub const fn width(self) -> Option<u32> {
        match self {
            Self::Thumb => Some(200),
            Self::Card => Some(400),
            Self::Detail => Some(800),
            Self::Full => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Thumb => "thumb",
            Self::Card => "card",
            Self::Detail => "detail",
            Self::Full => "full",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }
}

/// Encoded image formats.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Avif,
    Webp,
}

impl Format {
    pub const ALL: [Self; 2] = [Self::Avif, Self::Webp];

    #[must_use]
    pub const fn ext(self) -> &'static str {
        match self {
            Self::Avif => "avif",
            Self::Webp => "webp",
        }
    }

    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Avif => "image/avif",
            Self::Webp => "image/webp",
        }
    }

    #[must_use]
    pub fn from_ext(ext: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.ext() == ext)
    }
}

/// Which images get bytes ahead of being requested (D-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FetchPolicy {
    /// Only images a client actually requests.
    Lazy,
    /// Plus covers of library items and their related images.
    #[default]
    Library,
    /// Plus search results and trending items nobody has added.
    Eager,
}

/// Why bytes are fetched, in priority order (D-8).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FetchPriority {
    /// A client is rendering the image right now.
    Visible,
    /// Cover of an item in a library, list or play.
    Library,
    /// People, seasons, expansions of library items.
    Related,
    /// Search results and trending items.
    Catalog,
}

impl FetchPolicy {
    /// May bytes be fetched ahead of time for `priority`?
    #[must_use]
    pub fn allows(self, priority: FetchPriority) -> bool {
        match self {
            Self::Lazy => priority == FetchPriority::Visible,
            Self::Library => priority != FetchPriority::Catalog,
            Self::Eager => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    #[default]
    Fs,
    S3,
    /// In-memory, for tests and demos only.
    Memory,
}

/// Maximum source size; larger images are downscaled before variants are made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
}

impl FromStr for Pixels {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (w, h) = s
            .trim()
            .split_once(['x', 'X'])
            .ok_or_else(|| format!("expected WIDTHxHEIGHT, got `{s}`"))?;
        let parse = |v: &str| {
            v.trim()
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("invalid dimension in `{s}`"))
        };
        Ok(Self {
            width: parse(w)?,
            height: parse(h)?,
        })
    }
}

impl Serialize for Pixels {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{}x{}", self.width, self.height))
    }
}

impl<'de> Deserialize<'de> for Pixels {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// S3 settings and daily operation limits.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct S3Config {
    pub endpoint: Option<String>,
    pub bucket: String,
    pub region: String,
    /// Access key id; the secret comes from the environment only (`APP__STORAGE__S3__SECRET_ACCESS_KEY`).
    pub access_key_id: Option<String>,
    #[serde(skip_serializing)]
    pub secret_access_key: Option<String>,
    pub root: String,
    pub max_put_per_day: u64,
    pub max_get_per_day: u64,
    /// Estimated bytes served from S3 per day; when exceeded, redirect instead of proxying.
    #[schemars(with = "String")]
    pub max_egress_bytes_per_day: ByteSize,
}

impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("access_key_id", &self.access_key_id)
            .field(
                "secret_access_key",
                &self.secret_access_key.as_ref().map(|_| "***"),
            )
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: None,
            bucket: String::new(),
            region: "us-east-1".into(),
            access_key_id: None,
            secret_access_key: None,
            root: "/".into(),
            max_put_per_day: 50_000,
            max_get_per_day: 500_000,
            max_egress_bytes_per_day: ByteSize::gib(20),
        }
    }
}

/// The `storage` section of the configuration (D-8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct StorageConfig {
    pub backend: Backend,
    /// Root directory of the fs backend.
    pub fs_root: String,
    /// Hard cap for all stored bytes (pinned and cache tier).
    #[schemars(with = "String")]
    pub max_total_bytes: ByteSize,
    /// Start evicting above this fraction of the cap.
    pub high_watermark: f64,
    /// Evict down to this fraction of the cap.
    pub low_watermark: f64,
    /// fs only: never let free space on the volume drop below this.
    #[schemars(with = "String")]
    pub min_free_disk: MinFree,
    pub keep_originals: bool,
    /// Which variants to materialise.
    pub variants: Vec<Variant>,
    pub formats: Vec<Format>,
    #[schemars(with = "String")]
    pub max_source_pixels: Pixels,
    pub fetch_policy: FetchPolicy,
    /// Per-user limit for uploads (play photos, avatars).
    #[schemars(with = "String")]
    pub per_user_upload_quota: ByteSize,
    /// Limit for raw provider responses kept by the HTTP cache.
    #[schemars(with = "String")]
    pub http_cache_max_bytes: ByteSize,
    pub s3: S3Config,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: Backend::Fs,
            fs_root: "data/blobs".into(),
            max_total_bytes: ByteSize::gib(20),
            high_watermark: 0.90,
            low_watermark: 0.75,
            min_free_disk: MinFree::Bytes(ByteSize::gib(5)),
            keep_originals: false,
            variants: vec![Variant::Thumb, Variant::Card, Variant::Detail],
            formats: vec![Format::Avif, Format::Webp],
            max_source_pixels: Pixels {
                width: 4096,
                height: 4096,
            },
            fetch_policy: FetchPolicy::Library,
            per_user_upload_quota: ByteSize::gib(1),
            http_cache_max_bytes: ByteSize::gib(2),
            s3: S3Config::default(),
        }
    }
}

impl StorageConfig {
    /// # Errors
    /// A description of the first invalid setting.
    pub fn validate(&self) -> Result<(), String> {
        let wm_ok = |v: f64| v > 0.0 && v <= 1.0;
        if !wm_ok(self.high_watermark) || !wm_ok(self.low_watermark) {
            return Err("storage watermarks must be in (0, 1]".into());
        }
        if self.low_watermark >= self.high_watermark {
            return Err("storage.low_watermark must be below storage.high_watermark".into());
        }
        if self.max_total_bytes.0 == 0 {
            return Err("storage.max_total_bytes must be positive".into());
        }
        if self.http_cache_max_bytes > self.max_total_bytes {
            return Err("storage.http_cache_max_bytes must not exceed max_total_bytes".into());
        }
        if self.formats.is_empty() {
            return Err("storage.formats must list at least one format".into());
        }
        if self.backend == Backend::S3 && self.s3.bucket.is_empty() {
            return Err("storage.s3.bucket is required for the s3 backend".into());
        }
        Ok(())
    }

    /// Byte level above which eviction starts.
    #[must_use]
    pub fn high_bytes(&self) -> u64 {
        frac(self.max_total_bytes.0, self.high_watermark)
    }

    /// Byte level eviction aims for.
    #[must_use]
    pub fn low_bytes(&self) -> u64 {
        frac(self.max_total_bytes.0, self.low_watermark)
    }
}

// Byte counts fit in f64 well enough for watermark arithmetic.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn frac(total: u64, f: f64) -> u64 {
    (total as f64 * f).floor() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_byte_sizes() {
        assert_eq!("20GiB".parse::<ByteSize>().unwrap(), ByteSize::gib(20));
        assert_eq!("1.5 KiB".parse::<ByteSize>().unwrap(), ByteSize(1536));
        assert_eq!("2GB".parse::<ByteSize>().unwrap(), ByteSize(2_000_000_000));
        assert_eq!("42".parse::<ByteSize>().unwrap(), ByteSize(42));
        assert!("-1GiB".parse::<ByteSize>().is_err());
        assert!("lots".parse::<ByteSize>().is_err());
        assert_eq!(ByteSize::gib(2).to_string(), "2GiB");
    }

    #[test]
    fn parses_min_free() {
        assert_eq!(
            "10%".parse::<MinFree>().unwrap().required(1000),
            100,
            "fraction of the volume"
        );
        assert_eq!("5GiB".parse::<MinFree>().unwrap().required(1), 5 << 30);
        assert!("150%".parse::<MinFree>().is_err());
    }

    #[test]
    fn defaults_match_design() {
        let c = StorageConfig::default();
        assert_eq!(c.max_total_bytes, ByteSize::gib(20));
        assert_eq!(c.variants, [Variant::Thumb, Variant::Card, Variant::Detail]);
        assert_eq!(c.fetch_policy, FetchPolicy::Library);
        assert_eq!(c.s3.max_put_per_day, 50_000);
        assert!(!c.keep_originals);
        c.validate().unwrap();
    }

    #[test]
    fn deserialises_the_design_example() {
        let c: StorageConfig = toml::from_str(
            r#"
            backend = "fs"
            max_total_bytes = "20GiB"
            high_watermark = 0.9
            low_watermark = 0.75
            min_free_disk = "10%"
            keep_originals = false
            variants = ["thumb", "card", "detail"]
            max_source_pixels = "4096x4096"
            fetch_policy = "library"
            per_user_upload_quota = "1GiB"
            http_cache_max_bytes = "2GiB"
            [s3]
            max_put_per_day = 50000
            max_get_per_day = 500000
            max_egress_bytes_per_day = "20GiB"
            "#,
        )
        .unwrap();
        assert_eq!(c.min_free_disk, MinFree::Fraction(0.1));
        assert_eq!(c.s3.max_egress_bytes_per_day, ByteSize::gib(20));
        c.validate().unwrap();
    }

    #[test]
    fn rejects_inverted_watermarks() {
        let c = StorageConfig {
            low_watermark: 0.95,
            ..StorageConfig::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn fetch_policy_gates_priorities() {
        use FetchPriority::{Catalog, Library, Related, Visible};
        assert!(FetchPolicy::Lazy.allows(Visible));
        assert!(!FetchPolicy::Lazy.allows(Library));
        assert!(FetchPolicy::Library.allows(Related));
        assert!(!FetchPolicy::Library.allows(Catalog));
        assert!(FetchPolicy::Eager.allows(Catalog));
    }
}
