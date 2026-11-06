//! Image pipeline (D-7): decode with limits, cap the source size, derive variants as AVIF and
//! WebP, compute a thumbhash and the dominant colour. Pure CPU work: call it through
//! [`crate::pool::CpuPool`], never directly from async code.

use std::collections::HashMap;
use std::io::Cursor;

use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ImageReader, Limits, RgbaImage};
use thiserror::Error;

use crate::config::{Format, Pixels, Variant};

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("unsupported or corrupt image: {0}")]
    Decode(String),
    #[error("encode {0}: {1}")]
    Encode(&'static str, String),
}

/// What to produce.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub variants: Vec<Variant>,
    pub formats: Vec<Format>,
    pub max_source: Pixels,
    pub webp_quality: f32,
    pub avif_quality: u8,
    /// 1 (slow, small) ..= 10 (fast).
    pub avif_speed: u8,
}

impl PipelineConfig {
    #[must_use]
    pub fn new(variants: Vec<Variant>, formats: Vec<Format>, max_source: Pixels) -> Self {
        Self {
            variants,
            formats,
            max_source,
            webp_quality: 80.0,
            avif_quality: 70,
            avif_speed: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedVariant {
    pub variant: Variant,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Processed {
    /// blake3 of the source bytes: the asset's content address.
    pub hash: String,
    /// Source size after the `max_source` cap.
    pub width: u32,
    pub height: u32,
    pub thumbhash: Vec<u8>,
    /// `#rrggbb`.
    pub dominant_color: String,
    pub variants: Vec<EncodedVariant>,
}

/// Content address of some bytes.
#[must_use]
pub fn content_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Decoder limits: refuse absurd dimensions and allocations before decoding.
fn limits() -> Limits {
    let mut l = Limits::default();
    l.max_image_width = Some(20_000);
    l.max_image_height = Some(20_000);
    l.max_alloc = Some(1 << 30);
    l
}

fn decode(bytes: &[u8]) -> Result<RgbaImage, ImageError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    reader.limits(limits());
    let img = reader
        .decode()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    Ok(img.into_rgba8())
}

/// Largest size within `max_w` x `max_h` that keeps the aspect ratio (never upscales).
fn fit(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if w <= max_w && h <= max_h {
        return (w, h);
    }
    let scale = f64::min(
        f64::from(max_w) / f64::from(w),
        f64::from(max_h) / f64::from(h),
    );
    // Results are bounded by max_w/max_h, which are u32.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let scaled = |v: u32| ((f64::from(v) * scale).round() as u32).max(1);
    (scaled(w), scaled(h))
}

fn resize(src: &RgbaImage, w: u32, h: u32) -> Result<RgbaImage, ImageError> {
    if src.width() == w && src.height() == h {
        return Ok(src.clone());
    }
    let src = DynamicImage::ImageRgba8(src.clone());
    let mut dst = DynamicImage::ImageRgba8(RgbaImage::new(w, h));
    Resizer::new()
        .resize(
            &src,
            &mut dst,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3)),
        )
        .map_err(|e| ImageError::Encode("resize", e.to_string()))?;
    Ok(dst.into_rgba8())
}

fn encode(img: &RgbaImage, format: Format, cfg: &PipelineConfig) -> Result<Vec<u8>, ImageError> {
    match format {
        Format::Webp => Ok(
            webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height())
                .encode(cfg.webp_quality)
                .to_vec(),
        ),
        Format::Avif => {
            let mut out = Vec::new();
            let enc = image::codecs::avif::AvifEncoder::new_with_speed_quality(
                &mut out,
                cfg.avif_speed,
                cfg.avif_quality,
            );
            img.write_with_encoder(enc)
                .map_err(|e| ImageError::Encode("avif", e.to_string()))?;
            Ok(out)
        }
    }
}

/// Most common colour bucket (4 bits per channel) of a small copy, ignoring transparency;
/// the bucket's average is returned as `#rrggbb`.
fn dominant_color(small: &RgbaImage) -> String {
    let mut buckets: HashMap<u16, (u32, [u32; 3])> = HashMap::new();
    for p in small.pixels() {
        let [red, green, blue, alpha] = p.0;
        if alpha < 128 {
            continue;
        }
        let key = (u16::from(red >> 4) << 8) | (u16::from(green >> 4) << 4) | u16::from(blue >> 4);
        let entry = buckets.entry(key).or_insert((0, [0; 3]));
        entry.0 += 1;
        entry.1[0] += u32::from(red);
        entry.1[1] += u32::from(green);
        entry.1[2] += u32::from(blue);
    }
    let Some((_, (n, sum))) = buckets
        .into_iter()
        .max_by_key(|(key, (n, _))| (*n, std::cmp::Reverse(*key)))
    else {
        return "#000000".into();
    };
    let avg = |s: u32| u8::try_from(s / n).unwrap_or(u8::MAX);
    format!("#{:02x}{:02x}{:02x}", avg(sum[0]), avg(sum[1]), avg(sum[2]))
}

/// Run the whole pipeline on one source image.
///
/// # Errors
/// Undecodable input or an encoder failure.
pub fn process(bytes: &[u8], cfg: &PipelineConfig) -> Result<Processed, ImageError> {
    let hash = content_hash(bytes);
    let decoded = decode(bytes)?;
    let (w, h) = fit(
        decoded.width(),
        decoded.height(),
        cfg.max_source.width,
        cfg.max_source.height,
    );
    let source = resize(&decoded, w, h)?;
    drop(decoded);

    let (tw, th) = fit(w, h, 100, 100);
    let tiny = resize(&source, tw, th)?;
    let thumbhash = thumbhash::rgba_to_thumb_hash(tw as usize, th as usize, tiny.as_raw());
    let dominant_color = dominant_color(&tiny);

    let mut variants = Vec::new();
    for &variant in &cfg.variants {
        let (vw, vh) = match variant.width() {
            Some(max_w) => fit(w, h, max_w, u32::MAX),
            None => (w, h),
        };
        let img = resize(&source, vw, vh)?;
        for &format in &cfg.formats {
            variants.push(EncodedVariant {
                variant,
                format,
                width: vw,
                height: vh,
                bytes: encode(&img, format, cfg)?,
            });
        }
    }
    Ok(Processed {
        hash,
        width: w,
        height: h,
        thumbhash,
        dominant_color,
        variants,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A PNG test image: left half red, right half blue.
    pub(crate) fn sample_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_fn(w, h, |x, _| {
            if x < w / 2 {
                image::Rgba([220, 20, 20, 255])
            } else {
                image::Rgba([20, 20, 220, 255])
            }
        });
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn cfg() -> PipelineConfig {
        PipelineConfig::new(
            vec![Variant::Thumb, Variant::Card],
            vec![Format::Avif, Format::Webp],
            Pixels {
                width: 600,
                height: 600,
            },
        )
    }

    #[test]
    fn fit_keeps_aspect_and_never_upscales() {
        assert_eq!(fit(1000, 500, 200, u32::MAX), (200, 100));
        assert_eq!(fit(100, 50, 200, u32::MAX), (100, 50));
        assert_eq!(fit(4000, 8000, 4096, 4096), (2048, 4096));
    }

    #[test]
    fn produces_every_variant_in_every_format() {
        let p = process(&sample_png(1000, 1500), &cfg()).unwrap();
        // capped to 600 high
        assert_eq!((p.width, p.height), (400, 600));
        assert_eq!(p.variants.len(), 4);
        let thumb = p
            .variants
            .iter()
            .find(|v| v.variant == Variant::Thumb && v.format == Format::Webp)
            .unwrap();
        assert_eq!((thumb.width, thumb.height), (200, 300));
        assert_eq!(&thumb.bytes[8..12], b"WEBP");
        let avif = p
            .variants
            .iter()
            .find(|v| v.format == Format::Avif)
            .unwrap();
        assert_eq!(&avif.bytes[4..8], b"ftyp");
        assert!(!p.thumbhash.is_empty(), "thumbhash");
        assert_eq!(p.hash, content_hash(&sample_png(1000, 1500)));
    }

    #[test]
    fn dominant_colour_is_one_of_the_halves() {
        let p = process(&sample_png(64, 64), &cfg()).unwrap();
        assert!(
            p.dominant_color == "#dc1414" || p.dominant_color == "#1414dc",
            "{}",
            p.dominant_color
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            process(b"not an image", &cfg()),
            Err(ImageError::Decode(_))
        ));
    }
}
