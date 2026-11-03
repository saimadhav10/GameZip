use crate::{
    filesystem,
    model::{Candidate, Discovery},
};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone)]
enum Value {
    Text(String),
    Object(BTreeMap<String, Value>),
}

fn tokenize(text: &str) -> Result<Vec<String>, String> {
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' | '}' => tokens.push(c.to_string()),
            '"' => {
                let mut token = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    if c == '\\' && matches!(chars.peek(), Some('\\' | '"')) {
                        token.push(chars.next().unwrap());
                    } else {
                        token.push(c);
                    }
                }
                if !closed {
                    return Err("Unterminated manifest string".into());
                }
                // Quoted braces must remain text, so prefix all string tokens.
                tokens.push(format!("={token}"));
            }
            _ => return Err("Unexpected manifest syntax".into()),
        }
    }
    Ok(tokens)
}

fn parse(text: &str) -> Result<BTreeMap<String, Value>, String> {
    fn object(
        tokens: &[String],
        cursor: &mut usize,
        nested: bool,
        depth: usize,
    ) -> Result<BTreeMap<String, Value>, String> {
        if depth > 16 {
            return Err("Manifest nesting is too deep".into());
        }
        let mut values = BTreeMap::new();
        while *cursor < tokens.len() {
            if tokens[*cursor] == "}" {
                if !nested {
                    return Err("Unexpected closing brace".into());
                }
                *cursor += 1;
                return Ok(values);
            }
            let key = tokens[*cursor]
                .strip_prefix('=')
                .ok_or("Expected manifest key")?
                .to_string();
            *cursor += 1;
            let token = tokens.get(*cursor).ok_or("Missing manifest value")?;
            *cursor += 1;
            let value = if token == "{" {
                Value::Object(object(tokens, cursor, true, depth + 1)?)
            } else {
                Value::Text(
                    token
                        .strip_prefix('=')
                        .ok_or("Expected manifest value")?
                        .to_string(),
                )
            };
            values.insert(key, value);
        }
        if nested {
            return Err("Missing closing brace".into());
        }
        Ok(values)
    }
    object(&tokenize(text)?, &mut 0, false, 0)
}

fn text_value<'a>(object: &'a BTreeMap<String, Value>, key: &str) -> Option<&'a str> {
    match object.get(key)? {
        Value::Text(text) => Some(text),
        _ => None,
    }
}

fn read_manifest(path: &Path) -> Result<String, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
        return Err("Manifest exceeds size limit".into());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}

fn steam_libraries(root: &Path, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    let mut libraries = vec![root.to_path_buf()];
    let file = root.join("steamapps/libraryfolders.vdf");
    if !file.exists() {
        return libraries;
    }
    match read_manifest(&file).and_then(|s| parse(&s)) {
        Ok(data) => {
            if let Some(Value::Object(entries)) = data.get("libraryfolders") {
                for (key, value) in entries {
                    if key.parse::<u32>().is_err() {
                        continue;
                    }
                    let path = match value {
                        Value::Object(item) => text_value(item, "path"),
                        Value::Text(path) => Some(path.as_str()),
                    };
                    if let Some(path) = path {
                        libraries.push(PathBuf::from(path));
                    }
                }
            }
        }
        Err(e) => warnings.push(format!("Cannot read Steam libraries: {e}")),
    }
    libraries
}

pub fn steam_game(manifest: &str, library: &Path) -> Result<Option<Candidate>, String> {
    let data = parse(manifest)?;
    let Some(Value::Object(app)) = data.get("AppState") else {
        return Err("Missing AppState".into());
    };
    let name = text_value(app, "name").ok_or("Missing game name")?;
    let directory = text_value(app, "installdir").ok_or("Missing installation folder")?;
    let relative = Path::new(directory);
    if directory.is_empty()
        || !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("Invalid installation folder".into());
    }
    // Steam's fully installed flag is 4. Incomplete downloads are not installed games.
    let flags = text_value(app, "StateFlags")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    if flags & 4 == 0 {
        return Ok(None);
    }
    let path = library.join("steamapps/common").join(relative);
    if !path.is_dir() {
        return Ok(None);
    }
    Ok(Some(Candidate {
        name: name.into(),
        path: filesystem::display(&path),
        launcher: "Steam".into(),
    }))
}

pub fn discover() -> Discovery {
    let mut result = Discovery::default();
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        if let Ok(key) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(r"Software\Valve\Steam")
            && let Ok(path) = key.get_value::<String, _>("SteamPath")
        {
            roots.push(PathBuf::from(path));
        }
    }
    for env in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(path) = std::env::var_os(env) {
            roots.push(PathBuf::from(path).join("Steam"));
        }
    }
    let mut visited = HashSet::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for library in steam_libraries(&root, &mut result.warnings) {
            let Ok(canonical) = fs::canonicalize(&library) else {
                continue;
            };
            if !visited.insert(canonical) {
                continue;
            }
            let manifests = match fs::read_dir(library.join("steamapps")) {
                Ok(m) => m,
                Err(e) => {
                    result
                        .warnings
                        .push(format!("Cannot read {}: {e}", library.display()));
                    continue;
                }
            };
            for entry in manifests.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "acf") {
                    continue;
                }
                match read_manifest(&path).and_then(|s| steam_game(&s, &library)) {
                    Ok(Some(game)) => result.games.push(game),
                    Ok(None) => {}
                    Err(e) => result.warnings.push(format!(
                        "{}: {e}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )),
                }
            }
        }
    }
    if let Some(program_data) = std::env::var_os("PROGRAMDATA") {
        let directory = PathBuf::from(program_data).join("Epic/EpicGamesLauncher/Data/Manifests");
        if directory.is_dir() {
            match fs::read_dir(&directory) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_none_or(|e| e != "item") {
                            continue;
                        }
                        match read_manifest(&path).and_then(|s| epic_game(&s)) {
                            Ok(Some(game)) => result.games.push(game),
                            Ok(None) => {}
                            Err(e) => result
                                .warnings
                                .push(format!("Cannot read Epic manifest: {e}")),
                        }
                    }
                }
                Err(e) => result
                    .warnings
                    .push(format!("Cannot read Epic library: {e}")),
            }
        }
    }
    let mut paths = HashSet::new();
    result.games.retain(|g| paths.insert(g.path.to_lowercase()));
    result.games.sort_by_key(|a| a.name.to_lowercase());
    result
}

pub fn epic_game(manifest: &str) -> Result<Option<Candidate>, String> {
    let data: serde_json::Value = serde_json::from_str(manifest).map_err(|e| e.to_string())?;
    if data
        .get("bIsIncompleteInstall")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let name = data
        .get("DisplayName")
        .and_then(|v| v.as_str())
        .ok_or("Missing game name")?;
    let path = data
        .get("InstallLocation")
        .and_then(|v| v.as_str())
        .ok_or("Missing installation folder")?;
    if !Path::new(path).is_dir() {
        return Ok(None);
    }
    Ok(Some(Candidate {
        name: name.into(),
        path: path.into(),
        launcher: "Epic".into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_installed_games_but_rejects_downloads_and_path_traversal() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("steamapps/common/Example")).unwrap();
        let manifest =
            r#""AppState" { "name" "Test Game" "installdir" "Example" "StateFlags" "4" }"#;
        assert_eq!(
            steam_game(manifest, temp.path()).unwrap().unwrap().name,
            "Test Game"
        );
        assert!(
            steam_game(&manifest.replace("\"4\"", "\"2\""), temp.path())
                .unwrap()
                .is_none()
        );
        assert!(steam_game(&manifest.replace("Example", "../outside"), temp.path()).is_err());
    }
    #[test]
    fn malformed_manifest_is_an_error_not_a_partial_library() {
        assert!(parse(r#""AppState" { "name" "Missing brace""#).is_err());
        assert!(parse(r#""key" "unterminated"#).is_err());
    }
    #[test]
    fn epic_ignores_incomplete_and_missing_installations() {
        let temp = tempfile::tempdir().unwrap();
        let mut json = serde_json::json!({"DisplayName":"Example", "InstallLocation":temp.path(), "bIsIncompleteInstall":false});
        assert!(epic_game(&json.to_string()).unwrap().is_some());
        json["bIsIncompleteInstall"] = true.into();
        assert!(epic_game(&json.to_string()).unwrap().is_none());
    }
}
