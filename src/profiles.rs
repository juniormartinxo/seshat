use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSource {
    CliFlag,
    Environment,
    ProjectConfig,
    GlobalConfig,
    DirectoryBinding,
    CloakDefault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfile {
    pub name: String,
    pub source: ProfileSource,
}

impl ResolvedProfile {
    pub fn new(name: impl Into<String>, source: ProfileSource) -> Self {
        Self {
            name: name.into(),
            source,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloakCliHomes {
    pub codex_home: Option<PathBuf>,
    pub claude_config_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloakInstalledProfile {
    pub name: String,
    pub root_dir: PathBuf,
    pub cli_homes: CloakCliHomes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloakProfileDiscovery {
    pub root_dir: PathBuf,
    pub config_path: PathBuf,
    pub profiles_dir: PathBuf,
    pub default_profile: Option<String>,
    pub installed_profiles: Vec<CloakInstalledProfile>,
}

impl CloakProfileDiscovery {
    pub fn installed_profile(&self, name: &str) -> Option<&CloakInstalledProfile> {
        self.installed_profiles
            .iter()
            .find(|profile| profile.name == name)
    }
}

pub fn discover_cloak_profiles() -> Result<Option<CloakProfileDiscovery>> {
    let Some(home_dir) = current_home_dir() else {
        return Ok(None);
    };
    Ok(Some(discover_cloak_profiles_in(home_dir)?))
}

pub fn discover_cloak_profiles_in(home_dir: impl AsRef<Path>) -> Result<CloakProfileDiscovery> {
    let home_dir = home_dir.as_ref();
    let root_dir = home_dir.join(".config").join("cloak");
    let config_path = root_dir.join("config.toml");
    let profiles_dir = root_dir.join("profiles");
    let default_profile = fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| parse_default_profile(&content));
    let installed_profiles = read_installed_profiles(&profiles_dir)?;
    Ok(CloakProfileDiscovery {
        root_dir,
        config_path,
        profiles_dir,
        default_profile,
        installed_profiles,
    })
}

fn current_home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn read_installed_profiles(profiles_dir: &Path) -> Result<Vec<CloakInstalledProfile>> {
    if !profiles_dir.exists() {
        return Ok(Vec::new());
    }

    let mut installed_profiles = Vec::new();
    for entry in fs::read_dir(profiles_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let root_dir = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        installed_profiles.push(CloakInstalledProfile {
            name,
            cli_homes: CloakCliHomes {
                codex_home: existing_dir(root_dir.join("codex")),
                claude_config_dir: existing_dir(root_dir.join("claude")),
            },
            root_dir,
        });
    }

    installed_profiles.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(installed_profiles)
}

fn existing_dir(path: PathBuf) -> Option<PathBuf> {
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Some(path),
        _ => None,
    }
}

fn parse_default_profile(content: &str) -> Option<String> {
    let mut in_general_section = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_general_section = &line[1..line.len() - 1] == "general";
            continue;
        }
        if !in_general_section {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim() != "default_profile" {
            continue;
        }
        return parse_toml_string(value);
    }
    None
}

fn parse_toml_string(value: &str) -> Option<String> {
    let value = value.split('#').next()?.trim();
    if value.len() >= 2 {
        let first = value.as_bytes()[0];
        let last = value.as_bytes()[value.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return Some(value[1..value.len() - 1].to_string());
        }
    }
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discovery_reads_default_profile_from_config_toml() {
        let temp = tempdir().unwrap();
        let cloak_root = temp.path().join(".config").join("cloak");
        fs::create_dir_all(&cloak_root).unwrap();
        fs::write(
            cloak_root.join("config.toml"),
            r#"
[general]
default_profile = "amjr"

[cli.codex]
binary = "codex"
"#,
        )
        .unwrap();

        let discovery = discover_cloak_profiles_in(temp.path()).unwrap();

        assert_eq!(discovery.default_profile.as_deref(), Some("amjr"));
        assert_eq!(discovery.config_path, cloak_root.join("config.toml"));
    }

    #[test]
    fn discovery_lists_installed_profiles_and_cli_homes() {
        let temp = tempdir().unwrap();
        let profiles_dir = temp.path().join(".config").join("cloak").join("profiles");
        fs::create_dir_all(profiles_dir.join("samwise").join("codex")).unwrap();
        fs::create_dir_all(profiles_dir.join("samwise").join("claude")).unwrap();
        fs::create_dir_all(profiles_dir.join("alfred").join("codex")).unwrap();
        fs::write(profiles_dir.join("README.txt"), "ignore me").unwrap();

        let discovery = discover_cloak_profiles_in(temp.path()).unwrap();

        assert_eq!(
            discovery
                .installed_profiles
                .iter()
                .map(|profile| profile.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alfred", "samwise"]
        );

        let samwise = discovery.installed_profile("samwise").unwrap();
        assert_eq!(
            samwise.cli_homes.codex_home.as_deref(),
            Some(profiles_dir.join("samwise").join("codex").as_path())
        );
        assert_eq!(
            samwise.cli_homes.claude_config_dir.as_deref(),
            Some(profiles_dir.join("samwise").join("claude").as_path())
        );

        let alfred = discovery.installed_profile("alfred").unwrap();
        assert_eq!(
            alfred.cli_homes.codex_home.as_deref(),
            Some(profiles_dir.join("alfred").join("codex").as_path())
        );
        assert_eq!(alfred.cli_homes.claude_config_dir, None);
    }

    #[test]
    fn discovery_handles_missing_cloak_without_failing() {
        let temp = tempdir().unwrap();

        let discovery = discover_cloak_profiles_in(temp.path()).unwrap();

        assert_eq!(discovery.default_profile, None);
        assert!(discovery.installed_profiles.is_empty());
        assert_eq!(
            discovery.root_dir,
            temp.path().join(".config").join("cloak")
        );
    }
}
