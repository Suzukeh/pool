//! CPU テキストラスタ（ab_glyph）。M2: 単行・ASCII 確定、日本語は tofu。
//! M5 で fontdb + 整形（cosmic-text 等）に置き換える。

use ab_glyph::{Font, FontRef, Glyph, GlyphId, PxScale, ScaleFont, point};

/// 白＋α のビットマップ。
pub struct TextBitmap {
    pub width: u32,
    pub height: u32,
    /// RGBA8、白、α＝カバレッジ。
    pub rgba: Vec<u8>,
}

pub fn rasterize(font: &FontRef, body: &str, size_px: f32) -> TextBitmap {
    let scale = PxScale::from(size_px.max(1.0));
    let sf = font.as_scaled(scale).scale_factor();
    let ascent = font.ascent_unscaled() * sf.vertical;
    let descent = font.descent_unscaled() * sf.vertical;
    let height = ((ascent - descent).ceil() as u32).max(1) + 4;

    // 計測パス（unscaled 値を px 換算）
    let mut measured = 4.0f32;
    let mut prev: Option<GlyphId> = None;
    for c in body.chars() {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            measured += font.kern_unscaled(p, id) * sf.horizontal;
        }
        measured += font.h_advance_unscaled(id) * sf.horizontal;
        prev = Some(id);
    }
    let width = (measured.ceil() as u32).max(1) + 2;

    // 描画パス
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let mut caret_x = 2.0f32;
    let mut prev: Option<GlyphId> = None;
    for c in body.chars() {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            caret_x += font.kern_unscaled(p, id) * sf.horizontal;
        }
        let glyph = Glyph {
            id,
            scale,
            position: point(caret_x, 2.0 + ascent),
        };
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bb = outlined.px_bounds();
            outlined.draw(|x, y, v| {
                let px = (bb.min.x as u32 + x).min(width - 1);
                let py = (bb.min.y as u32 + y).min(height - 1);
                let idx = ((py * width + px) * 4) as usize;
                let a = (v * 255.0) as u8;
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = rgba[idx + 3].max(a);
            });
        }
        caret_x += font.h_advance_unscaled(id) * sf.horizontal;
        prev = Some(id);
    }
    TextBitmap {
        width,
        height,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_font() -> FontRef<'static> {
        FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap()
    }

    #[test]
    fn ascii_produces_ink() {
        let bmp = rasterize(&test_font(), "Hi", 40.0);
        assert!(bmp.width > 10 && bmp.height > 10);
        assert!(bmp.rgba.chunks_exact(4).any(|p| p[3] > 0));
    }
}
