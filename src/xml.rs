use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use zed::LanguageServerId;
use zed_extension_api::{self as zed, settings::LspSettings, Result};

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
}

zed::register_extension!(XmlExtension);

#[cfg(test)]
mod tests {
    use super::*;

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
