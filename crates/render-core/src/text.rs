//! CPU テキストラスタ（ab_glyph）。単行・左→右。
//!
//! フォントスタック：同梱 DejaVu（ASCII/Latin）＋システム探索。
//! 文字ごとに最初にグリフを持つフォントを使う（豆腐回避）。
//! M5 で整形（cosmic-text 等）に置き換え予定。

use ab_glyph::{Font, FontRef, Glyph, GlyphId, PxScale, ScaleFont, point};
use std::path::{Path, PathBuf};

/// 白＋α のビットマップ。
pub struct TextBitmap {
    pub width: u32,
    pub height: u32,
    /// RGBA8、白、α＝カバレッジ。
    pub rgba: Vec<u8>,
}

fn is_loadable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("ttf") | Some("otf") | Some("ttc")
    )
}

/// 候補ディレクトリを浅く走査してフォント候補を集める（上限付き）。
fn collect_candidates(dirs: &[PathBuf], out: &mut Vec<PathBuf>, cap: usize) {
    let mut stack: Vec<PathBuf> = dirs.to_vec();
    while let Some(dir) = stack.pop() {
        if out.len() >= cap {
            return;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                subdirs.push(p);
            } else if p.is_file() && is_loadable(&p) {
                if let Ok(meta) = entry.metadata() {
                    if meta.len() < 80_000_000 {
                        out.push(p);
                    }
                }
                if out.len() >= cap {
                    return;
                }
            }
        }
        stack.extend(subdirs);
    }
}

fn leak_bytes(path: &Path) -> Option<&'static [u8]> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 128 {
        return None;
    }
    Some(Box::leak(data.into_boxed_slice()))
}

/// 'あ' を持つか（日本語カバレッジの粗い判定）。
fn covers_japanese(font: &FontRef) -> bool {
    font.glyph_id('あ').0 != 0
}

/// フォントスタックを構築する。[0] は必ず同梱 DejaVu。
pub fn load_font_stack() -> Vec<FontRef<'static>> {
    let mut stack = Vec::new();
    let bundled: &'static [u8] = include_bytes!("../assets/DejaVuSans.ttf");
    if let Ok(f) = FontRef::try_from_slice(bundled) {
        stack.push(f);
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(extra) = std::env::var_os("POOL_FONT_PATHS") {
        dirs.extend(std::env::split_paths(&extra));
    } else {
        #[cfg(target_os = "linux")]
        dirs.push(PathBuf::from("/usr/share/fonts"));
        #[cfg(target_os = "macos")]
        {
            dirs.push(PathBuf::from("/System/Library/Fonts"));
            dirs.push(PathBuf::from("/Library/Fonts"));
        }
        #[cfg(target_os = "windows")]
        {
            if let Some(windir) = std::env::var_os("WINDIR") {
                dirs.push(PathBuf::from(windir).join("Fonts"));
            }
        }
    }
    let mut candidates = Vec::new();
    collect_candidates(&dirs, &mut candidates, 64);
    candidates.sort();
    for path in candidates {
        if stack.len() >= 8 {
            break;
        }
        let Some(bytes) = leak_bytes(&path) else {
            continue;
        };
        let is_ttc = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("ttc"))
            .unwrap_or(false);
        if is_ttc {
            // TTC は 'あ' を持つ面だけ採用（最大 16 面プローブ）
            for index in 0..16 {
                match FontRef::try_from_slice_and_index(bytes, index) {
                    Ok(f) if covers_japanese(&f) => {
                        stack.push(f);
                        break;
                    }
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        } else if let Ok(f) = FontRef::try_from_slice(bytes) {
            stack.push(f);
        }
    }
    stack
}

pub fn rasterize(stack: &[FontRef], body: &str, size_px: f32) -> TextBitmap {
    let empty = TextBitmap {
        width: 1,
        height: 1,
        rgba: vec![0, 0, 0, 0],
    };
    let Some(primary) = stack.first().cloned() else {
        return empty;
    };
    let scale = PxScale::from(size_px.max(1.0));
    let psf = primary.as_scaled(scale).scale_factor();
    let ascent = primary.ascent_unscaled() * psf.vertical;
    let descent = primary.descent_unscaled() * psf.vertical;
    let height = ((ascent - descent).ceil() as u32).max(1) + 4;

    struct Item {
        fi: usize,
        id: GlyphId,
        adv: f32,
        kern: f32,
    }
    let mut items = Vec::new();
    let mut prev: Option<(usize, GlyphId)> = None;
    for c in body.chars() {
        let fi = stack.iter().position(|f| f.glyph_id(c).0 != 0).unwrap_or(0);
        let f = stack[fi].clone();
        let sf = f.as_scaled(scale).scale_factor();
        let id = f.glyph_id(c);
        let mut kern = 0.0;
        if let Some((pfi, pid)) = prev {
            if pfi == fi {
                kern = f.kern_unscaled(pid, id) * sf.horizontal;
            }
        }
        items.push(Item {
            fi,
            id,
            adv: f.h_advance_unscaled(id) * sf.horizontal,
            kern,
        });
        prev = Some((fi, id));
    }
    let width = (items.iter().map(|i| i.adv + i.kern).sum::<f32>() + 6.0)
        .ceil()
        .max(1.0) as u32;

    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let mut caret_x = 2.0f32;
    for it in &items {
        caret_x += it.kern;
        let f = stack[it.fi].clone();
        let glyph = Glyph {
            id: it.id,
            scale,
            position: point(caret_x, 2.0 + ascent),
        };
        if let Some(outlined) = f.outline_glyph(glyph) {
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
        caret_x += it.adv;
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

    fn bundled() -> FontRef<'static> {
        FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap()
    }

    #[test]
    fn ascii_produces_ink() {
        let bmp = rasterize(&[bundled()], "Hi", 40.0);
        assert!(bmp.width > 10 && bmp.height > 10);
        assert!(bmp.rgba.chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn bundled_lacks_japanese() {
        // 前提確認：DejaVu に 'あ' はない（フォールバックの存在意義）
        assert_eq!(bundled().glyph_id('あ').0, 0);
        assert_ne!(bundled().glyph_id('A').0, 0);
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    fn bundled() -> FontRef<'static> {
        FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap()
    }

    #[test]
    fn missing_glyph_draws_tofu_without_panic() {
        // DejaVu のみ：'あ' は .notdef の箱になる（欠字の可視化）
        let bmp = rasterize(&[bundled()], "Hiあ", 40.0);
        let hi = rasterize(&[bundled()], "Hi", 40.0);
        assert!(bmp.width > hi.width);
        assert!(bmp.rgba.chunks_exact(4).any(|p| p[3] > 0));
    }
}
