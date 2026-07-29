use std::fs;
use zed::LanguageServerId;
use zed_extension_api::{self as zed, Result};

struct XmlExtension {
    cached_binary_path: Option<String>,
}

static LS_NAME: &str = "lemminx";

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
            if fs::metadata(path).map_or(false, |stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let release = zed::latest_github_release(
            "redhat-developer/vscode-xml",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (platform, arch) = zed::current_platform();

        // See: https://github.com/redhat-developer/vscode-xml/releases
        let artifact = match (platform, arch) {
            (zed::Os::Mac, zed::Architecture::Aarch64) => "osx-aarch_64",
            (zed::Os::Mac, zed::Architecture::X8664) => "osx-x86_64",
            (zed::Os::Linux, zed::Architecture::X8664) => "linux",
            (zed::Os::Linux, zed::Architecture::Aarch64) => {
                return Err("unsupported platform: Linux aarch64.\nSee https://github.com/redhat-developer/vscode-xml/issues/930".into())
            }
            // TODO: When Windows aarch64 builds are available use those
            (zed::Os::Windows, _) => "win32",
            (_, zed::Architecture::X86) => return Err("unsupported architecture: 32bit x86".into()),
        };
        let binary_name = format!("{}-{}", LS_NAME, artifact);
        let asset_name = format!("{}.zip", binary_name);

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no asset found matching {:?}", asset_name))?;

        let version_dir = format!("{}-{}", LS_NAME, release.version);
        let extension = match platform {
            zed::Os::Windows => ".exe",
            _ => "",
        };
        let binary_path = format!("{}/{}{}", version_dir, binary_name, extension);

        if !fs::metadata(&binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(
                &asset.download_url,
                &version_dir,
                zed::DownloadedFileType::Zip,
            )
            .map_err(|e| format!("failed to download file: {e}"))?;

            let entries =
                fs::read_dir(".").map_err(|e| format!("failed to list working directory {e}"))?;
            for entry in entries {
                let entry = entry.map_err(|e| format!("failed to load directory entry {e}"))?;
                if entry.file_name().to_str() != Some(&version_dir) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
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
        Ok(zed::Command {
            command: self.language_server_binary_path(language_server_id, worktree)?,
            args: Default::default(),
            env: Default::default(),
        })
    }
}

zed::register_extension!(XmlExtension);
