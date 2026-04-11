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

    pub fn contains_profile(&self, name: &str) -> bool {
        self.installed_profile(name).is_some()
    }
}

pub fn resolve_profile_precedence(
    base_path: impl AsRef<Path>,
    cli_profile: Option<&str>,
    environment_profile: Option<&str>,
    project_profile: Option<&str>,
    global_profile: Option<&str>,
    cloak: Option<&CloakProfileDiscovery>,
) -> Option<ResolvedProfile> {
    if let Some(profile) = non_empty_profile(cli_profile) {
        return Some(ResolvedProfile::new(profile, ProfileSource::CliFlag));
    }
    if let Some(profile) = non_empty_profile(environment_profile) {
        return Some(ResolvedProfile::new(profile, ProfileSource::Environment));
    }
    if let Some(profile) = non_empty_profile(project_profile) {
        return Some(ResolvedProfile::new(profile, ProfileSource::ProjectConfig));
    }
    if let Some(profile) = find_directory_profile_binding(base_path, cloak) {
        return Some(ResolvedProfile::new(
            profile,
            ProfileSource::DirectoryBinding,
        ));
    }
    if let Some(profile) = cloak
        .and_then(|discovery| non_empty_profile(discovery.default_profile.as_deref()))
        .filter(|profile| cloak_profile_exists(cloak, profile))
    {
        return Some(ResolvedProfile::new(profile, ProfileSource::CloakDefault));
    }
    non_empty_profile(global_profile)
        .map(|profile| ResolvedProfile::new(profile, ProfileSource::GlobalConfig))
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

fn non_empty_profile(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn find_directory_profile_binding(
    base_path: impl AsRef<Path>,
    cloak: Option<&CloakProfileDiscovery>,
) -> Option<String> {
    let mut current = base_path.as_ref().canonicalize().ok().or_else(|| {
        let path = base_path.as_ref();
        if path.exists() {
            Some(path.to_path_buf())
        } else {
            path.parent().map(Path::to_path_buf)
        }
    });

    while let Some(path) = current {
        let binding_path = path.join(".cloak");
        if let Ok(content) = fs::read_to_string(&binding_path) {
            if let Some(profile) = parse_profile_binding(&content).filter(|profile| {
                cloak
                    .as_ref()
                    .map(|discovery| discovery.contains_profile(profile))
                    .unwrap_or(true)
            }) {
                return Some(profile);
            }
        }
        current = path.parent().map(Path::to_path_buf);
    }
    None
}

fn parse_profile_binding(content: &str) -> Option<String> {
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim() != "profile" {
            continue;
        }
        return parse_toml_string(value);
    }
    None
}

fn cloak_profile_exists(cloak: Option<&CloakProfileDiscovery>, profile: &str) -> bool {
    cloak
        .map(|discovery| discovery.contains_profile(profile))
        .unwrap_or(false)
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

    #[test]
    fn resolution_uses_directory_binding_before_cloak_default() {
        let temp = tempdir().unwrap();
        let project_root = temp.path().join("workspace").join("project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(project_root.join(".cloak"), "profile = \"samwise\"").unwrap();
        let profiles_dir = temp.path().join(".config").join("cloak").join("profiles");
        fs::create_dir_all(profiles_dir.join("samwise").join("codex")).unwrap();
        fs::create_dir_all(profiles_dir.join("amjr").join("codex")).unwrap();
        fs::write(
            temp.path()
                .join(".config")
                .join("cloak")
                .join("config.toml"),
            "[general]\ndefault_profile = \"amjr\"\n",
        )
        .unwrap();

        let discovery = discover_cloak_profiles_in(temp.path()).unwrap();
        let resolved =
            resolve_profile_precedence(&project_root, None, None, None, None, Some(&discovery))
                .unwrap();

        assert_eq!(
            resolved,
            ResolvedProfile::new("samwise", ProfileSource::DirectoryBinding)
        );
    }

    #[test]
    fn resolution_falls_back_to_cloak_default_when_binding_is_missing() {
        let temp = tempdir().unwrap();
        let project_root = temp.path().join("workspace").join("project");
        fs::create_dir_all(&project_root).unwrap();
        let profiles_dir = temp.path().join(".config").join("cloak").join("profiles");
        fs::create_dir_all(profiles_dir.join("amjr").join("codex")).unwrap();
        fs::write(
            temp.path()
                .join(".config")
                .join("cloak")
                .join("config.toml"),
            "[general]\ndefault_profile = \"amjr\"\n",
        )
        .unwrap();

        let discovery = discover_cloak_profiles_in(temp.path()).unwrap();
        let resolved =
            resolve_profile_precedence(&project_root, None, None, None, None, Some(&discovery))
                .unwrap();

        assert_eq!(
            resolved,
            ResolvedProfile::new("amjr", ProfileSource::CloakDefault)
        );
    }

    #[test]
    fn resolution_uses_closest_directory_binding() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let project_root = workspace.join("apps").join("project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(workspace.join(".cloak"), "profile = \"amjr\"").unwrap();
        fs::write(project_root.join(".cloak"), "profile = \"samwise\"").unwrap();
        let profiles_dir = temp.path().join(".config").join("cloak").join("profiles");
        fs::create_dir_all(profiles_dir.join("amjr").join("codex")).unwrap();
        fs::create_dir_all(profiles_dir.join("samwise").join("codex")).unwrap();

        let discovery = discover_cloak_profiles_in(temp.path()).unwrap();
        let resolved = resolve_profile_precedence(
            project_root.join("src"),
            None,
            None,
            None,
            None,
            Some(&discovery),
        )
        .unwrap();

        assert_eq!(
            resolved,
            ResolvedProfile::new("samwise", ProfileSource::DirectoryBinding)
        );
    }

    #[test]
    fn resolution_ignores_unknown_directory_binding_and_falls_back() {
        let temp = tempdir().unwrap();
        let project_root = temp.path().join("workspace").join("project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(project_root.join(".cloak"), "profile = \"missing\"").unwrap();
        let profiles_dir = temp.path().join(".config").join("cloak").join("profiles");
        fs::create_dir_all(profiles_dir.join("amjr").join("codex")).unwrap();
        fs::write(
            temp.path()
                .join(".config")
                .join("cloak")
                .join("config.toml"),
            "[general]\ndefault_profile = \"amjr\"\n",
        )
        .unwrap();

        let discovery = discover_cloak_profiles_in(temp.path()).unwrap();
        let resolved = resolve_profile_precedence(
            &project_root,
            None,
            None,
            None,
            Some("global-profile"),
            Some(&discovery),
        )
        .unwrap();

        assert_eq!(
            resolved,
            ResolvedProfile::new("amjr", ProfileSource::CloakDefault)
        );
    }

    #[test]
    fn resolution_preserves_explicit_precedence_over_cloak_sources() {
        let temp = tempdir().unwrap();
        let project_root = temp.path().join("workspace").join("project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(project_root.join(".cloak"), "profile = \"samwise\"").unwrap();
        let profiles_dir = temp.path().join(".config").join("cloak").join("profiles");
        fs::create_dir_all(profiles_dir.join("samwise").join("codex")).unwrap();
        fs::create_dir_all(profiles_dir.join("amjr").join("codex")).unwrap();
        fs::write(
            temp.path()
                .join(".config")
                .join("cloak")
                .join("config.toml"),
            "[general]\ndefault_profile = \"amjr\"\n",
        )
        .unwrap();

        let discovery = discover_cloak_profiles_in(temp.path()).unwrap();

        let cli = resolve_profile_precedence(
            &project_root,
            Some("cli-profile"),
            Some("env-profile"),
            Some("project-profile"),
            Some("global-profile"),
            Some(&discovery),
        )
        .unwrap();
        assert_eq!(
            cli,
            ResolvedProfile::new("cli-profile", ProfileSource::CliFlag)
        );

        let env = resolve_profile_precedence(
            &project_root,
            None,
            Some("env-profile"),
            Some("project-profile"),
            Some("global-profile"),
            Some(&discovery),
        )
        .unwrap();
        assert_eq!(
            env,
            ResolvedProfile::new("env-profile", ProfileSource::Environment)
        );

        let project = resolve_profile_precedence(
            &project_root,
            None,
            None,
            Some("project-profile"),
            Some("global-profile"),
            Some(&discovery),
        )
        .unwrap();
        assert_eq!(
            project,
            ResolvedProfile::new("project-profile", ProfileSource::ProjectConfig)
        );
    }
}
