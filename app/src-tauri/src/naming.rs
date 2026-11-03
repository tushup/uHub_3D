use std::collections::HashSet;
use std::path::Path;

use crate::error::LibError;
use crate::model::AttrKind;

/// Trim, turn spaces into `_`, drop chars outside `[A-Za-z0-9_]`.
/// Returns `None` when nothing usable remains.
pub fn sanitize_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Sanitize each name, replace empty results with `Material_<index+1>`
/// and make repeats unique with `_2`, `_3`, ... suffixes.
pub fn unique_names(raw: &[String]) -> Vec<String> {
    let mut used: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(raw.len());
    for (i, name) in raw.iter().enumerate() {
        let base = sanitize_name(name).unwrap_or_else(|| format!("Material_{}", i + 1));
        let mut candidate = base.clone();
        let mut n = 2;
        while used.contains(&candidate) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        used.insert(candidate.clone());
        out.push(candidate);
    }
    out
}

/// Lower-cased texture extension when supported, else `UnsupportedFormat`.
pub fn texture_ext(path: &Path) -> Result<String, LibError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match ext.as_deref() {
        Some(e @ ("jpg" | "jpeg" | "png" | "tga")) => Ok(e.to_string()),
        _ => Err(LibError::UnsupportedFormat(path.display().to_string())),
    }
}

fn kind_name(kind: AttrKind) -> &'static str {
    match kind {
        AttrKind::BaseColor => "BaseColor",
        AttrKind::Metallic => "Metallic",
        AttrKind::Roughness => "Roughness",
        AttrKind::Specular => "Specular",
        AttrKind::Normal => "Normal",
        AttrKind::Height => "Height",
        AttrKind::AO => "AO",
        AttrKind::Opacity => "Opacity",
        AttrKind::Emissive => "Emissive",
    }
}

pub fn texture_file_name(asset: &str, material: Option<&str>, kind: AttrKind, ext: &str) -> String {
    match material {
        Some(m) => format!("{asset}_{m}_{}.{ext}", kind_name(kind)),
        None => format!("{asset}_{}.{ext}", kind_name(kind)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::error::LibError;
    use crate::model::AttrKind;

    #[test]
    fn sanitize_basic() {
        assert_eq!(sanitize_name(" Old Chair! "), Some("Old_Chair".into()));
    }

    #[test]
    fn sanitize_empty() {
        assert_eq!(sanitize_name("!!"), None);
    }

    #[test]
    fn unique_names_dedupes() {
        assert_eq!(
            unique_names(&["Wood".into(), "Wood".into(), "".into(), "Mat.001".into()]),
            vec!["Wood", "Wood_2", "Material_3", "Mat001"]
        );
    }

    #[test]
    fn texture_ext_lowercases() {
        assert_eq!(texture_ext(Path::new("D:/Téxtures/Wood Base.PNG")).unwrap(), "png");
    }

    #[test]
    fn texture_ext_rejects_exr() {
        assert!(matches!(texture_ext(Path::new("a.exr")), Err(LibError::UnsupportedFormat(_))));
    }

    #[test]
    fn texture_names() {
        assert_eq!(texture_file_name("Chair", Some("Seat"), AttrKind::BaseColor, "png"), "Chair_Seat_BaseColor.png");
        assert_eq!(texture_file_name("Oak", None, AttrKind::Roughness, "jpg"), "Oak_Roughness.jpg");
    }
}
