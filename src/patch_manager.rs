use std::fs;
use std::path::{Path, PathBuf};

use crate::Patch;

pub fn patches_dir() -> PathBuf {
    let base = dirs();
    let dir = base.join("patches");
    if !dir.exists() {
        fs::create_dir_all(&dir).expect("Failed to create patches directory");
    }
    dir
}

pub fn save_patch(patch: &Patch, name: &str) -> Result<PathBuf, PatchError> {
    let path = patches_dir().join(format!("{}.toml", sanitize(name)));
    let toml = toml::to_string_pretty(patch).map_err(|e| PatchError::Serialize(e.to_string()))?;
    fs::write(&path, toml).map_err(|e| PatchError::Io(e.to_string()))?;
    Ok(path)
}

pub fn load_patch(name: &str) -> Result<Patch, PatchError> {
    let path = patches_dir().join(format!("{}.toml", sanitize(name)));
    load_patch_from_path(&path)
}

pub fn load_patch_from_path(path: &Path) -> Result<Patch, PatchError> {
    let toml = fs::read_to_string(path).map_err(|e| PatchError::Io(e.to_string()))?;
    toml::from_str(&toml).map_err(|e| PatchError::Deserialize(e.to_string()))
}

pub fn list_patches() -> Vec<String> {
    let dir = patches_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![];
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? == "toml" {
                Some(path.file_stem()?.to_str()?.to_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

pub fn delete_patch(name: &str) -> Result<(), PatchError> {
    let path = patches_dir().join(format!("{}.toml", sanitize(name)));
    fs::remove_file(path).map_err(|e| PatchError::Io(e.to_string()))
}

#[derive(Debug)]
pub enum PatchError {
    Io(String),
    Serialize(String),
    Deserialize(String),
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchError::Io(e) => write!(f, "IO error: {e}"),
            PatchError::Serialize(e) => write!(f, "Serialize error: {e}"),
            PatchError::Deserialize(e) => write!(f, "Deserialize error: {e}"),
        }
    }
}

impl std::error::Error for PatchError {}

// Patches are stored in a platform-appropriate directory:
// macOS & Linux:   ~/.config/gregory/patches/
// Windows: %APPDATA%\gregory\patches\
fn dirs() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(appdata).join("gregory")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".config").join("gregory")
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Patch;

    #[test]
    fn test_save_and_load_roundtrip() {
        let patch = Patch::default();
        let name = "test_patch_roundtrip";
        save_patch(&patch, name).expect("Failed to save patch");
        let loaded = load_patch(name).expect("Failed to load patch");
        assert_eq!(patch, loaded);
        delete_patch(name).expect("Failed to delete test patch");
    }

    #[test]
    fn test_list_patches_includes_saved() {
        let patch = Patch::default();
        let name = "test_patch_list";
        save_patch(&patch, name).expect("Failed to save");
        let names = list_patches();
        assert!(names.contains(&name.to_owned()));
        delete_patch(name).expect("Failed to delete");
    }

    #[test]
    fn test_load_nonexistent_returns_error() {
        let result = load_patch("this_patch_does_not_exist_xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_strips_bad_chars() {
        let name = "my/patch:name*here";
        let s = sanitize(name);
        assert!(!s.contains('/'));
        assert!(!s.contains(':'));
        assert!(!s.contains('*'));
    }
}
