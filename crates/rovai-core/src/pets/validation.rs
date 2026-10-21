// Apache-2.0; spacering-net/codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f.
use image::{ImageReader,ImageFormat};
use super::model::*;
const MIN_SPRITE_BYTES:usize=1024;
const MAX_SPRITE_BYTES:usize=16*1024*1024;
const MAX_SPRITE_ROWS:u32=32;
/// Detected sprite-sheet container.
#[derive(Debug, Clone, Copy)]
pub enum SpriteFormat {
    Png,
    Webp,
}

impl SpriteFormat {
    pub const fn mime(self) -> &'static str {
        match self {
            SpriteFormat::Png => "image/png",
            SpriteFormat::Webp => "image/webp",
        }
    }

    pub const fn filename(self) -> &'static str {
        // We always *store* under the canonical Codex name so directories are
        // round-trippable. PNG uploads are renamed at write time.
        SPRITESHEET_FILENAME
    }
}

/// Decode the image header just enough to verify the sprite-sheet contract.
/// Returns the detected format on success.
pub fn validate_spritesheet(bytes: &[u8]) -> Result<SpriteFormat, anyhow::Error> {
    if bytes.len() < MIN_SPRITE_BYTES {
        return Err(anyhow::Error::msg(
            "Spritesheet payload is too small to be valid.",
        ));
    }
    if bytes.len() > MAX_SPRITE_BYTES {
        return Err(anyhow::Error::msg(format!(
            "Spritesheet payload exceeds {} MiB cap.",
            MAX_SPRITE_BYTES / (1024 * 1024)
        )));
    }

    let cursor = std::io::Cursor::new(bytes);
    let reader = ImageReader::new(cursor)
        .with_guessed_format()
        .map_err(|e| anyhow::Error::msg(format!("Cannot read sprite header: {e}")))?;
    let format = reader.format().ok_or_else(|| {
        anyhow::Error::msg("Spritesheet must be a PNG or WebP image.")
    })?;
    let detected = match format {
        ImageFormat::Png => SpriteFormat::Png,
        ImageFormat::WebP => SpriteFormat::Webp,
        _ => {
            return Err(anyhow::Error::msg(
                "Spritesheet must be a PNG or WebP image.",
            ));
        }
    };

    let img = reader
        .decode()
        .map_err(|e| anyhow::Error::msg(format!("Cannot decode sprite: {e}")))?;
    check_sprite_dimensions(img.width(), img.height())?;
    if !img.color().has_alpha() {
        return Err(anyhow::Error::msg(
            "Spritesheet must contain an alpha channel (transparent background).",
        ));
    }

    Ok(detected)
}

/// Verify a decoded sprite sheet's pixel grid. The width is fixed at 8 columns
/// (`SPRITE_SHEET_WIDTH`); the height must be a whole number of 208px rows,
/// covering at least the base animation states and no more than the row cap.
/// This is where format evolution is absorbed: v1 sheets are 9 rows (1872px)
/// and v2 sheets are 11 rows (2288px), and both pass.
fn check_sprite_dimensions(width: u32, height: u32) -> Result<(), anyhow::Error> {
    if width != SPRITE_SHEET_WIDTH {
        return Err(anyhow::Error::msg(format!(
            "Spritesheet must be {SPRITE_SHEET_WIDTH}px wide (8 columns); got width {width}."
        )));
    }
    if height == 0 || !height.is_multiple_of(SPRITE_FRAME_HEIGHT) {
        return Err(anyhow::Error::msg(format!(
            "Spritesheet height must be a whole multiple of {SPRITE_FRAME_HEIGHT}px rows; got {height}."
        )));
    }
    let rows = height / SPRITE_FRAME_HEIGHT;
    if rows < SPRITE_GRID_ROWS {
        return Err(anyhow::Error::msg(format!(
            "Spritesheet must have at least {SPRITE_GRID_ROWS} rows ({}px); got {rows} rows ({height}px).",
            SPRITE_GRID_ROWS * SPRITE_FRAME_HEIGHT
        )));
    }
    if rows > MAX_SPRITE_ROWS {
        return Err(anyhow::Error::msg(format!(
            "Spritesheet has too many rows ({rows}); the maximum is {MAX_SPRITE_ROWS}."
        )));
    }
    Ok(())
}

/// Slug-validate a pet id. Returns `Err` on invalid input. Defense in depth:
/// the frontend slugifies but we must independently reject malformed ids
/// before they touch the filesystem.
pub fn validate_pet_id(id: &str) -> Result<(), anyhow::Error> {
    if id.is_empty() {
        return Err(anyhow::Error::msg("Pet id is required."));
    }
    if id.len() > 64 {
        return Err(anyhow::Error::msg(
            "Pet id must be at most 64 characters.",
        ));
    }
    let valid = id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !valid {
        return Err(anyhow::Error::msg(
            "Pet id may only contain lowercase letters, digits, '-' and '_'.",
        ));
    }
    if id.starts_with('-') || id.ends_with('-') || id.starts_with('.') {
        return Err(anyhow::Error::msg(
            "Pet id cannot start with '.' or '-' / end with '-'.",
        ));
    }
    Ok(())
}

