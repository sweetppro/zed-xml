use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use zed::LanguageServerId;
use zed_extension_api::{self as zed, serde_json::Value, settings::LspSettings, Result};

mod lemminx_release;

struct XmlExtension {
    cached_binary_path: Option<String>,
}

static LS_NAME: &str = "lemminx";
static LS_REPO: &str = "redhat-developer/vscode-xml";

impl XmlExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        if let Some(path) = worktree.which(LS_NAME) {
            return Ok(path);
        }

        if let Some(path) = &self.cached_binary_path {
            if is_file(path) {
                return Ok(path.clone());
            }
        }

        let binary_path = install_binary(language_server_id).map_err(|e| {
            format!(
                "failed to install {LS_NAME} from {LS_REPO} {}: {e}\n\
                 Install LemMinX yourself and point `lsp.lemminx.binary.path` at it in your settings: \
                 https://github.com/sweetppro/zed-xml#language-server",
                lemminx_release::TAG
            )
        })?;

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
}

/// Downloads the pinned release of the binary for this platform and checks it against the
/// SHA-256 Red Hat publishes with it. Nothing reaches its final directory unverified.
fn install_binary(language_server_id: &LanguageServerId) -> Result<String> {
    let (platform, arch) = zed::current_platform();
    let binary_name = binary_name(platform, arch)?;
    let expected_sha256 = expected_sha256(binary_name)?;

    let version_dir = format!("{}-{}", LS_NAME, lemminx_release::TAG);
    let extension = match platform {
        zed::Os::Windows => ".exe",
        _ => "",
    };
    let binary_file = format!("{}{}", binary_name, extension);
    let binary_path = format!("{}/{}", version_dir, binary_file);

    if is_file(&binary_path) {
        return Ok(binary_path);
    }

    zed::set_language_server_installation_status(
        language_server_id,
        &zed::LanguageServerInstallationStatus::CheckingForUpdate,
    );
    let release = zed::github_release_by_tag_name(LS_REPO, lemminx_release::TAG)?;

    let asset_name = format!("{}.zip", binary_name);
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| format!("no asset found matching {:?}", asset_name))?;

    zed::set_language_server_installation_status(
        language_server_id,
        &zed::LanguageServerInstallationStatus::Downloading,
    );

    let download_dir = format!("{}.download", version_dir);
    fs::remove_dir_all(&download_dir).ok();
    zed::download_file(
        &asset.download_url,
        &download_dir,
        zed::DownloadedFileType::Zip,
    )
    .map_err(|e| format!("failed to download file: {e}"))?;

    let verified = sha256_file(&format!("{}/{}", download_dir, binary_file)).and_then(|actual| {
        if actual == expected_sha256 {
            Ok(())
        } else {
            Err(format!(
                "checksum mismatch for {binary_file}: expected {expected_sha256}, got {actual}"
            ))
        }
    });
    if let Err(e) = verified {
        fs::remove_dir_all(&download_dir).ok();
        return Err(e);
    }

    fs::remove_dir_all(&version_dir).ok();
    fs::rename(&download_dir, &version_dir)
        .map_err(|e| format!("failed to move {download_dir} to {version_dir}: {e}"))?;
    zed::make_file_executable(&binary_path)?;

    let entries = fs::read_dir(".").map_err(|e| format!("failed to list working directory {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to load directory entry {e}"))?;
        if entry.file_name().to_str() != Some(&version_dir) {
            fs::remove_dir_all(entry.path()).ok();
        }
    }

    Ok(binary_path)
}

/// The name of the release asset, without `.zip`, and of the executable inside it.
/// See: https://github.com/redhat-developer/vscode-xml/releases
fn binary_name(platform: zed::Os, arch: zed::Architecture) -> Result<&'static str> {
    match (platform, arch) {
        (zed::Os::Mac, zed::Architecture::Aarch64) => Ok("lemminx-osx-aarch_64"),
        (zed::Os::Mac, zed::Architecture::X8664) => Ok("lemminx-osx-x86_64"),
        (zed::Os::Linux, zed::Architecture::Aarch64) => Ok("lemminx-linux-aarch_64"),
        (zed::Os::Linux, zed::Architecture::X8664) => Ok("lemminx-linux-x86_64"),
        // There is no Windows aarch64 build; the x86_64 one runs under emulation.
        (zed::Os::Windows, zed::Architecture::X8664 | zed::Architecture::Aarch64) => {
            Ok("lemminx-win32")
        }
        (_, zed::Architecture::X86) => Err("unsupported architecture: 32bit x86".into()),
    }
}

fn expected_sha256(binary_name: &str) -> Result<&'static str> {
    lemminx_release::SHA256
        .iter()
        .find(|(name, _)| *name == binary_name)
        .map(|(_, sha256)| *sha256)
        .ok_or_else(|| format!("no checksum recorded for {binary_name}"))
}

fn sha256_file(path: &str) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| format!("failed to open {path}: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("failed to read {path}: {e}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn is_file(path: &str) -> bool {
    fs::metadata(path).is_ok_and(|stat| stat.is_file())
}

/// The `lsp.lemminx.settings` of the worktree, with project-relative paths made absolute.
fn workspace_settings(worktree: &zed::Worktree) -> Value {
    let mut settings = LspSettings::for_worktree(LS_NAME, worktree)
        .ok()
        .and_then(|settings| settings.settings)
        .unwrap_or_else(|| Value::Object(Default::default()));
    resolve_relative_paths(&mut settings, &worktree.root_path());
    settings
}

/// `/x`, `\x`, `C:x` or a URI: anything LemMinX can open without knowing the project.
fn is_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with('\\')
        || path.contains("://")
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

fn absolute(path: &str, root: &str) -> String {
    if is_absolute(path) {
        return path.to_string();
    }
    let root = root.trim_end_matches(['/', '\\']);
    let relative = path.trim_start_matches("./");
    format!("{root}/{relative}")
}

/// LemMinX resolves catalogs and associated schemas against its own working directory, not
/// the project, so the VS Code client rewrites them first. This does the same, so a project's
/// settings can say `schemas/catalog.xml` instead of a path that only exists on one machine.
fn resolve_relative_paths(settings: &mut Value, root: &str) {
    let Some(xml) = settings.get_mut("xml") else {
        return;
    };
    if let Some(catalogs) = xml.get_mut("catalogs").and_then(Value::as_array_mut) {
        for catalog in catalogs.iter_mut() {
            if let Some(path) = catalog.as_str() {
                *catalog = Value::String(absolute(path, root));
            }
        }
    }
    if let Some(associations) = xml
        .get_mut("fileAssociations")
        .and_then(Value::as_array_mut)
    {
        for association in associations.iter_mut() {
            if let Some(system_id) = association.get_mut("systemId") {
                if let Some(path) = system_id.as_str() {
                    *system_id = Value::String(absolute(path, root));
                }
            }
        }
    }
}

/// LemMinX reads its settings at start-up from `initializationOptions.settings`.
fn initialization_options(user: Option<Value>, settings: Value) -> Value {
    let mut options = match user {
        Some(Value::Object(map)) => Value::Object(map),
        _ => Value::Object(Default::default()),
    };
    if let Value::Object(map) = &mut options {
        map.entry("settings").or_insert(settings);
    }
    options
}

impl zed::Extension for XmlExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree(LS_NAME, worktree)
            .ok()
            .and_then(|settings| settings.binary);
        let (path, args, env) = match binary {
            Some(binary) => (binary.path, binary.arguments, binary.env),
            None => (None, None, None),
        };
        let command = match path {
            Some(path) => path,
            None => self.language_server_binary_path(language_server_id, worktree)?,
        };

        Ok(zed::Command {
            command,
            args: args.unwrap_or_default(),
            env: env.unwrap_or_default().into_iter().collect(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<Value>> {
        let user = LspSettings::for_worktree(LS_NAME, worktree)
            .ok()
            .and_then(|settings| settings.initialization_options);
        Ok(Some(initialization_options(
            user,
            workspace_settings(worktree),
        )))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<Value>> {
        Ok(Some(workspace_settings(worktree)))
    }
}

zed::register_extension!(XmlExtension);

#[cfg(test)]
mod tests {
    use super::*;
    use zed_extension_api::serde_json::json;

    #[test]
    fn makes_project_paths_absolute_and_leaves_the_rest() {
        let mut settings = json!({
            "xml": {
                "catalogs": ["schemas/catalog.xml", "./more.xml", "/etc/xml/catalog", "C:\\xml\\c.xml"],
                "fileAssociations": [
                    { "pattern": "**/*.xml", "systemId": "schemas/a.xsd" },
                    { "pattern": "**/*.svg", "systemId": "https://example.com/svg.xsd" }
                ],
                "validation": { "enabled": true }
            }
        });
        resolve_relative_paths(&mut settings, "/home/dev/project/");
        assert_eq!(
            settings["xml"]["catalogs"],
            json!([
                "/home/dev/project/schemas/catalog.xml",
                "/home/dev/project/more.xml",
                "/etc/xml/catalog",
                "C:\\xml\\c.xml"
            ])
        );
        assert_eq!(
            settings["xml"]["fileAssociations"][0]["systemId"],
            "/home/dev/project/schemas/a.xsd"
        );
        assert_eq!(
            settings["xml"]["fileAssociations"][1]["systemId"],
            "https://example.com/svg.xsd"
        );
        assert_eq!(settings["xml"]["validation"], json!({ "enabled": true }));
    }

    #[test]
    fn joins_a_windows_root() {
        assert_eq!(
            absolute("schemas/catalog.xml", "C:\\dev\\project"),
            "C:\\dev\\project/schemas/catalog.xml"
        );
    }

    #[test]
    fn leaves_settings_without_xml_alone() {
        let mut settings = json!({ "other": ["a.xml"] });
        resolve_relative_paths(&mut settings, "/root");
        assert_eq!(settings, json!({ "other": ["a.xml"] }));
    }

    #[test]
    fn passes_the_settings_in_the_initialization_options() {
        let settings = json!({ "xml": { "catalogs": ["/c.xml"] } });
        assert_eq!(
            initialization_options(None, settings.clone()),
            json!({ "settings": { "xml": { "catalogs": ["/c.xml"] } } })
        );
        assert_eq!(
            initialization_options(Some(json!({ "extendedClientCapabilities": {} })), settings),
            json!({ "extendedClientCapabilities": {}, "settings": { "xml": { "catalogs": ["/c.xml"] } } })
        );
    }

    #[test]
    fn keeps_settings_the_user_put_in_the_initialization_options() {
        let user = json!({ "settings": { "xml": { "catalogs": ["/mine.xml"] } } });
        assert_eq!(initialization_options(Some(user.clone()), json!({})), user);
    }

    #[test]
    fn records_a_checksum_for_every_published_binary() {
        let platforms = [
            (zed::Os::Mac, zed::Architecture::Aarch64),
            (zed::Os::Mac, zed::Architecture::X8664),
            (zed::Os::Linux, zed::Architecture::Aarch64),
            (zed::Os::Linux, zed::Architecture::X8664),
            (zed::Os::Windows, zed::Architecture::X8664),
            (zed::Os::Windows, zed::Architecture::Aarch64),
        ];
        for (platform, arch) in platforms {
            let name = binary_name(platform, arch).unwrap();
            let sha256 = expected_sha256(name).unwrap();
            assert_eq!(sha256.len(), 64, "{name}");
            assert!(sha256.bytes().all(|b| b.is_ascii_hexdigit()), "{name}");
        }
        assert!(binary_name(zed::Os::Windows, zed::Architecture::X86).is_err());
    }

    #[test]
    fn hashes_a_file() {
        let path = std::env::temp_dir().join("zed-xml-sha256-test");
        fs::write(&path, b"abc").unwrap();
        let sha256 = sha256_file(path.to_str().unwrap()).unwrap();
        fs::remove_file(&path).ok();
        assert_eq!(
            sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
