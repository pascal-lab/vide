use std::{fs, path::Path};

use zed::settings::LspSettings;
use zed_extension_api::{
    self as zed, Architecture, Command, DownloadedFileType, GithubReleaseOptions, LanguageServerId,
    LanguageServerInstallationStatus, Os, Result, Worktree, serde_json,
};

const LANGUAGE_SERVER_ID: &str = "vide";
const RELEASE_REPO: &str = "pascal-lab/vide";

struct VideBinary {
    path: String,
    args: Vec<String>,
}

#[derive(Default)]
struct VideExtension {
    cached_binary_path: Option<String>,
}

impl VideExtension {
    fn language_server_binary(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<VideBinary> {
        let mut args = Vec::new();
        if let Ok(settings) = LspSettings::for_worktree(language_server_id.as_ref(), worktree) {
            if let Some(binary) = settings.binary {
                args = binary.arguments.unwrap_or_default();
                if let Some(path) = binary.path {
                    return Ok(VideBinary { path, args });
                }
            }
        }

        let path = worktree
            .which(binary_name(current_os()))
            .unwrap_or(self.zed_managed_binary_path(language_server_id)?);

        Ok(VideBinary { path, args })
    }

    fn zed_managed_binary_path(&mut self, language_server_id: &LanguageServerId) -> Result<String> {
        if let Some(path) = &self.cached_binary_path {
            if is_file(path) {
                return Ok(path.clone());
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            RELEASE_REPO,
            GithubReleaseOptions { require_assets: true, pre_release: false },
        )?;

        let (os, arch) = zed::current_platform();
        let asset_name = release_asset_name(os, arch)?;
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no Vide release asset found matching {asset_name:?}"))?;

        let version_dir = format!("vide-{}", release.version);
        let binary_path = binary_path(&version_dir, os);

        if !is_file(&binary_path) {
            zed::set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::Downloading,
            );

            prepare_version_dir(&version_dir)?;
            zed::download_file(&asset.download_url, &binary_path, DownloadedFileType::Uncompressed)
                .map_err(|error| format!("failed to download {asset_name}: {error}"))?;
            ensure_executable(&binary_path, os)?;

            prune_old_downloads(&version_dir)?;
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
}

impl zed::Extension for VideExtension {
    fn new() -> Self {
        Self::default()
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        if language_server_id.as_ref() != LANGUAGE_SERVER_ID {
            return Err(format!("unknown language server `{language_server_id}`"));
        }

        let binary = self.language_server_binary(language_server_id, worktree)?;
        Ok(Command { command: binary.path, args: binary.args, env: Vec::new() })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        let initialization_options =
            LspSettings::for_worktree(language_server_id.as_ref(), worktree)
                .ok()
                .and_then(|settings| settings.initialization_options.clone())
                .unwrap_or_default();

        Ok(Some(initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|settings| settings.settings.clone())
            .unwrap_or_default();

        Ok(Some(settings))
    }
}

fn current_os() -> Os {
    let (os, _) = zed::current_platform();
    os
}

fn binary_name(os: Os) -> &'static str {
    match os {
        Os::Windows => "vide.exe",
        Os::Mac | Os::Linux => "vide",
    }
}

fn release_asset_name(os: Os, arch: Architecture) -> Result<String> {
    match (os, arch) {
        (Os::Linux, Architecture::X8664) => Ok("vide-linux-x64".to_string()),
        (Os::Linux, Architecture::Aarch64) => Ok("vide-linux-arm64".to_string()),
        (Os::Mac, Architecture::Aarch64) => Ok("vide-darwin-arm64".to_string()),
        (Os::Windows, Architecture::X8664) => Ok("vide-win32-x64.exe".to_string()),
        (os, arch) => Err(format!("Vide does not publish a release asset for {os:?}-{arch:?}")),
    }
}

fn binary_path(version_dir: &str, os: Os) -> String {
    format!("{version_dir}/{}", binary_name(os))
}

fn is_file(path: &str) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

fn prepare_version_dir(version_dir: &str) -> Result<()> {
    if fs::metadata(version_dir).is_ok_and(|metadata| metadata.is_file()) {
        fs::remove_file(version_dir)
            .map_err(|error| format!("failed to remove old Vide binary {version_dir}: {error}"))?;
    }

    fs::create_dir_all(version_dir)
        .map_err(|error| format!("failed to create Vide binary directory {version_dir}: {error}"))
}

fn ensure_executable(path: &str, os: Os) -> Result<()> {
    if os != Os::Windows && Path::new(path).exists() {
        zed::make_file_executable(path)?;
    }
    Ok(())
}

fn prune_old_downloads(current_dir: &str) -> Result<()> {
    let entries =
        fs::read_dir(".").map_err(|error| format!("failed to list extension data: {error}"))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("failed to read extension data entry: {error}"))?;
        if entry.file_name().to_str() != Some(current_dir) {
            fs::remove_dir_all(entry.path()).ok();
        }
    }
    Ok(())
}

zed::register_extension!(VideExtension);
