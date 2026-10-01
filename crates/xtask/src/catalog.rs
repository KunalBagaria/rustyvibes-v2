//! `assets/soundpacks/catalog.json`: which packs ship, in which order, and
//! where their original recordings live.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Menu categories, in display order.
pub const CATEGORIES: [&str; 3] = ["linear", "tactile", "clicky"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Mechvibes,
    Kbsim,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub variant: String,
    pub category: String,
    pub color: String,
    pub credit: String,
    pub kind: SourceKind,
    pub dir: PathBuf,
    pub order: u16,
}

#[derive(Clone, Debug)]
pub struct Catalog {
    pub default: String,
    pub entries: Vec<Entry>,
}

pub fn load(path: &Path) -> crate::Result<Catalog> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text, path.parent().unwrap_or(Path::new(".")))
}

/// Parses catalog JSON; source directories are resolved against `base`.
pub fn parse(text: &str, base: &Path) -> crate::Result<Catalog> {
    let root: Value = serde_json::from_str(text).map_err(|e| format!("catalog: {e}"))?;
    let default = string(&root, "default", "catalog")?;
    let packs =
        root.get("packs").and_then(Value::as_array).ok_or("catalog: missing \"packs\" array")?;
    let mut entries: Vec<Entry> = Vec::new();
    for (index, pack) in packs.iter().enumerate() {
        let id = string(pack, "id", "catalog pack")?;
        let ctx = format!("pack {id:?}");
        if entries.iter().any(|e| e.id == id) {
            return Err(format!("{ctx}: duplicate id"));
        }
        if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(format!("{ctx}: ids may only contain a-z, 0-9 and '-'"));
        }
        let category = string(pack, "category", &ctx)?;
        if !CATEGORIES.contains(&category.as_str()) {
            return Err(format!("{ctx}: unknown category {category:?}"));
        }
        let color = string(pack, "color", &ctx)?;
        if !is_hex_color(&color) {
            return Err(format!("{ctx}: color must look like #RRGGBB"));
        }
        let source = pack.get("source").ok_or_else(|| format!("{ctx}: missing source"))?;
        let kind = match string(source, "kind", &ctx)?.as_str() {
            "mechvibes" => SourceKind::Mechvibes,
            "kbsim" => SourceKind::Kbsim,
            other => return Err(format!("{ctx}: unknown source kind {other:?}")),
        };
        entries.push(Entry {
            name: string(pack, "name", &ctx)?,
            variant: pack.get("variant").and_then(Value::as_str).unwrap_or_default().to_owned(),
            credit: string(pack, "credit", &ctx)?,
            dir: base.join(string(source, "dir", &ctx)?),
            order: u16::try_from(index).map_err(|_| "catalog: too many packs")?,
            id,
            category,
            color,
            kind,
        });
    }
    if !entries.iter().any(|e| e.id == default) {
        return Err(format!("catalog: default pack {default:?} is not listed"));
    }
    Ok(Catalog { default, entries })
}

fn string(value: &Value, key: &str, ctx: &str) -> crate::Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{ctx}: missing string field {key:?}"))
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{ "default": "b", "packs": [
      {"id":"a","name":"A","category":"linear","color":"#112233","credit":"X","source":{"kind":"kbsim","dir":"kbsim/a"}},
      {"id":"b","name":"B","variant":"PBT","category":"clicky","color":"#AABBCC","credit":"Y","source":{"kind":"mechvibes","dir":"mechvibes/b"}}
    ] }"##;

    #[test]
    fn parses_entries_in_order() {
        let catalog = parse(SAMPLE, Path::new("/assets")).unwrap();
        assert_eq!(catalog.default, "b");
        assert_eq!(catalog.entries.len(), 2);
        assert_eq!(catalog.entries[0].dir, Path::new("/assets/kbsim/a"));
        assert_eq!(catalog.entries[0].variant, "");
        assert_eq!(catalog.entries[1].variant, "PBT");
        assert_eq!(catalog.entries[1].order, 1);
        assert_eq!(catalog.entries[1].kind, SourceKind::Mechvibes);
    }

    #[test]
    fn rejects_bad_catalogs() {
        let cases = [
            ("\"default\": \"b\"", "\"default\": \"zz\"", "not listed"),
            ("\"category\":\"linear\"", "\"category\":\"mushy\"", "unknown category"),
            ("\"color\":\"#112233\"", "\"color\":\"red\"", "#RRGGBB"),
            ("\"id\":\"b\"", "\"id\":\"a\"", "duplicate"),
            ("\"id\":\"b\"", "\"id\":\"B!\"", "a-z"),
            ("\"kind\":\"kbsim\"", "\"kind\":\"other\"", "unknown source kind"),
        ];
        for (needle, replacement, expected) in cases {
            let text = SAMPLE.replacen(needle, replacement, 1);
            let err = parse(&text, Path::new("/")).unwrap_err();
            assert!(err.contains(expected), "{needle} → {err}");
        }
    }

    #[test]
    fn shipped_catalog_is_valid() {
        let path = crate::workspace_root().join("assets/soundpacks/catalog.json");
        let catalog = load(&path).unwrap();
        assert_eq!(catalog.entries.len(), 21);
        for entry in &catalog.entries {
            assert!(entry.dir.is_dir(), "{} is missing {}", entry.id, entry.dir.display());
        }
    }
}
