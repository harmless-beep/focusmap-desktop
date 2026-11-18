//! Font loading.
//!
//! FocusMap does not ship a font binary. It uses the same clean sans-serif the
//! OS already has (Segoe UI on Windows, Inter if the user installed it, DejaVu on
//! Linux), which keeps the download small and the text native.

use std::path::PathBuf;

pub struct FontSet {
    pub regular: Vec<u8>,
    pub bold: Option<Vec<u8>>,
    pub name: String,
}

pub fn load() -> Option<FontSet> {
    for (regular, bold, name) in candidates() {
        if let Ok(bytes) = std::fs::read(&regular) {
            return Some(FontSet {
                regular: bytes,
                bold: std::fs::read(&bold).ok(),
                name,
            });
        }
    }
    None
}

fn candidates() -> Vec<(PathBuf, PathBuf, String)> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        let windir = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        let fonts = windir.join("Fonts");
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let user_fonts = PathBuf::from(local).join("Microsoft\\Windows\\Fonts");
            out.push((
                user_fonts.join("Inter-Regular.ttf"),
                user_fonts.join("Inter-Bold.ttf"),
                "Inter".into(),
            ));
            out.push((
                user_fonts.join("InterVariable.ttf"),
                user_fonts.join("InterVariable.ttf"),
                "Inter".into(),
            ));
        }
        out.push((
            fonts.join("segoeui.ttf"),
            fonts.join("segoeuib.ttf"),
            "Segoe UI".into(),
        ));
        out.push((
            fonts.join("seguisb.ttf"),
            fonts.join("segoeuib.ttf"),
            "Segoe UI Semibold".into(),
        ));
        out.push((
            fonts.join("arial.ttf"),
            fonts.join("arialbd.ttf"),
            "Arial".into(),
        ));
    }
    #[cfg(target_os = "macos")]
    {
        out.push((
            PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf"),
            PathBuf::from("/System/Library/Fonts/Supplemental/Arial Bold.ttf"),
            "Arial".into(),
        ));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        out.push((
            PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
            "DejaVu Sans".into(),
        ));
        out.push((
            PathBuf::from("/usr/share/fonts/TTF/DejaVuSans.ttf"),
            PathBuf::from("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"),
            "DejaVu Sans".into(),
        ));
    }
    out
}
