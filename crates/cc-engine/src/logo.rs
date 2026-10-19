//! Logo firmy do popisového poľa výkresov: PNG alebo JPEG zo súboru, zmenšené a
//! položené na bielu (priehľadnosť).

use std::path::{Path, PathBuf};

use cc_i18n::tr;
use cc_out_pdf::pdf::RgbImage;

/// Najväčší súbor loga.
const MAX_BYTES: u64 = 20 * 1024 * 1024;
/// Najväčší rozmer obrázka pred zmenšením.
const MAX_SIDE: u32 = 10_000;
/// Logo sa zmenší do tohto rozmeru v bodoch (≈ 300 dpi v políčku 62 × 22 mm).
const FIT: (u32, u32) = (720, 260);

/// Cesta k logu: relatívna od priečinka projektu.
pub(crate) fn resolve(base: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

/// Načíta logo. Chyba je text pre používateľa.
pub(crate) fn load(base: &Path, path: &str) -> Result<RgbImage, String> {
    let file = resolve(base, path);
    let shown = file.display().to_string();
    let err =
        |e: &dyn std::fmt::Display| tr!("engine.logo.error", file = &shown, error = e.to_string());
    let len = std::fs::metadata(&file).map_err(|e| err(&e))?.len();
    if len > MAX_BYTES {
        return Err(tr!("engine.logo.too_big", file = &shown));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    let mut reader = image::ImageReader::open(&file)
        .map_err(|e| err(&e))?
        .with_guessed_format()
        .map_err(|e| err(&e))?;
    reader.limits(limits);
    let img = reader.decode().map_err(|e| {
        tr!(
            "engine.logo.not_image",
            file = &shown,
            error = e.to_string()
        )
    })?;
    let img = if img.width() > FIT.0 || img.height() > FIT.1 {
        img.resize(FIT.0, FIT.1, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    if width == 0 || height == 0 {
        return Err(tr!("engine.logo.empty", file = &shown));
    }
    // priehľadné časti na bielu
    let rgb = rgba
        .pixels()
        .flat_map(|p| {
            let [r, g, b, a] = p.0;
            let a = u16::from(a);
            let over = |v: u8| ((u16::from(v) * a + 255 * (255 - a) + 127) / 255) as u8;
            [over(r), over(g), over(b)]
        })
        .collect();
    Ok(RgbImage { width, height, rgb })
}
