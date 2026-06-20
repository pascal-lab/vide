#![recursion_limit = "512"]

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};

const VSCODE_SCHEMA_CONSTANTS_PATH: &str = "editors/vscode/src/generated/projectConfigSchema.ts";
const VSCODE_CONFIGURATION_PATH: &str = "editors/vscode/src/generated/configuration.ts";
const VSCODE_PACKAGE_PATH: &str = "editors/vscode/package.json";
const USER_CONFIG_SCHEMA_PATH: &str = "/schemas/v1/user-config.schema.json";

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace_root = workspace_root()?;

    match cli.command {
        Some(XtaskCommand::GenerateConfigArtifacts) => write_config_artifacts(&workspace_root),
        Some(XtaskCommand::CheckConfigArtifacts) => check_config_artifacts(&workspace_root),
        Some(XtaskCommand::GenerateSchemas) => write_schemas(&workspace_root),
        Some(XtaskCommand::CheckSchemas) => check_schemas(&workspace_root),
        Some(XtaskCommand::Server(server)) => run_server_command(&workspace_root, server),
        Some(XtaskCommand::Vscode(vscode)) => run_vscode_command(&workspace_root, vscode),
        None => {
            Cli::command().print_help()?;
            eprintln!();
            Ok(())
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "xtask", bin_name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    command: Option<XtaskCommand>,
}

#[derive(Debug, Subcommand)]
enum XtaskCommand {
    GenerateConfigArtifacts,
    CheckConfigArtifacts,
    #[command(alias = "generate-manifest-schema")]
    GenerateSchemas,
    #[command(alias = "check-manifest-schema")]
    CheckSchemas,
    Server(ServerArgs),
    Vscode(VscodeArgs),
}

#[derive(Debug, Args)]
struct ServerArgs {
    #[command(subcommand)]
    command: ServerCommand,
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    Build(ServerBuildArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
struct ServerBuildArgs {
    #[arg(long, value_enum, default_value = "debug")]
    profile: ExtensionBuildProfile,
    #[arg(long)]
    cargo_target: Option<String>,
    #[arg(long)]
    alpine_linker: bool,
    #[arg(long)]
    profile_trace: bool,
}

#[derive(Debug, Args)]
struct VscodeArgs {
    #[command(subcommand)]
    command: VscodeCommand,
}

#[derive(Debug, Subcommand)]
enum VscodeCommand {
    PrepareServer(VscodePrepareServerArgs),
}

#[derive(Debug, PartialEq, Eq, Args)]
struct VscodePrepareServerArgs {
    #[arg(long, value_enum)]
    target: VscodeServerTarget,
    #[arg(long, value_enum, default_value = "release")]
    profile: ExtensionBuildProfile,
    #[arg(long, value_enum, default_value = "build")]
    server: ExtensionServerMode,
    #[arg(long)]
    profile_trace: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ExtensionBuildProfile {
    Debug,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ExtensionServerMode {
    Build,
    Prebuilt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
enum VscodeServerTarget {
    AlpineArm64,
    AlpineX64,
    DarwinArm64,
    LinuxArm64,
    LinuxX64,
    Win32X64,
}

impl VscodeServerTarget {
    fn folder(self) -> &'static str {
        match self {
            VscodeServerTarget::AlpineArm64 => "alpine-arm64",
            VscodeServerTarget::AlpineX64 => "alpine-x64",
            VscodeServerTarget::DarwinArm64 => "darwin-arm64",
            VscodeServerTarget::LinuxArm64 => "linux-arm64",
            VscodeServerTarget::LinuxX64 => "linux-x64",
            VscodeServerTarget::Win32X64 => "win32-x64",
        }
    }

    fn binary_file(self) -> &'static str {
        if self.is_windows() { "vide.exe" } else { "vide" }
    }

    fn cargo_target(self) -> Option<&'static str> {
        match self {
            VscodeServerTarget::AlpineArm64 => Some("aarch64-unknown-linux-musl"),
            VscodeServerTarget::AlpineX64 => Some("x86_64-unknown-linux-musl"),
            _ => None,
        }
    }

    fn is_windows(self) -> bool {
        matches!(self, VscodeServerTarget::Win32X64)
    }

    fn requires_alpine_linker(self) -> bool {
        matches!(self, VscodeServerTarget::AlpineArm64 | VscodeServerTarget::AlpineX64)
    }
}

fn run_vscode_command(workspace_root: &Path, args: VscodeArgs) -> Result<()> {
    match args.command {
        VscodeCommand::PrepareServer(args) => prepare_vscode_server(workspace_root, args),
    }
}

fn run_server_command(workspace_root: &Path, args: ServerArgs) -> Result<()> {
    match args.command {
        ServerCommand::Build(args) => {
            let server_path = build_server(workspace_root, &args)?;
            println!("{}", server_path.display());
            Ok(())
        }
    }
}

fn prepare_vscode_server(workspace_root: &Path, args: VscodePrepareServerArgs) -> Result<()> {
    let server_path = ensure_vscode_server_binary(
        workspace_root,
        args.target,
        args.profile,
        args.server,
        args.profile_trace,
    )?;
    println!("{}", server_path.display());
    Ok(())
}

fn ensure_vscode_server_binary(
    workspace_root: &Path,
    target: VscodeServerTarget,
    profile: ExtensionBuildProfile,
    server_mode: ExtensionServerMode,
    profile_trace: bool,
) -> Result<PathBuf> {
    let server_path = vscode_target_server_path(workspace_root, target);
    if server_mode == ExtensionServerMode::Prebuilt {
        if server_path.exists() {
            ensure_vscode_server_executable(&server_path, target)?;
            return Ok(server_path);
        }
        bail!("missing prebuilt server binary: {}", server_path.display());
    }

    let host_target = host_vscode_server_target()?;
    let cargo_target = target.cargo_target();
    if target != host_target && cargo_target.is_none() {
        bail!(
            "missing bundled server binary: {}\n\
             tip: run packaging on a matching native runner or copy the target binary first.",
            server_path.display()
        );
    }

    let build_args = server_build_args_for_vscode_target(target, profile, profile_trace);
    let source_path = build_server(workspace_root, &build_args)?;
    let parent = server_path.parent().context("VS Code server output path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::copy(&source_path, &server_path).with_context(|| {
        format!(
            "failed to copy server binary from {} to {}",
            source_path.display(),
            server_path.display()
        )
    })?;
    ensure_vscode_server_executable(&server_path, target)?;

    Ok(server_path)
}

fn build_server(workspace_root: &Path, args: &ServerBuildArgs) -> Result<PathBuf> {
    if let Some(cargo_target) = args.cargo_target.as_deref() {
        run_command(
            "rustup",
            &["target".to_owned(), "add".to_owned(), cargo_target.to_owned()],
            workspace_root,
            &[],
        )?;
    }

    run_command("cargo", &cargo_build_args(args), workspace_root, &cargo_build_env_updates(args))?;

    Ok(cargo_output_dir(workspace_root, args).join(server_binary_file(args)))
}

fn host_vscode_server_target() -> Result<VscodeServerTarget> {
    match (env::consts::OS, env::consts::ARCH) {
        ("linux", "aarch64") => Ok(VscodeServerTarget::LinuxArm64),
        ("linux", "x86_64") => Ok(VscodeServerTarget::LinuxX64),
        ("macos", "aarch64") => Ok(VscodeServerTarget::DarwinArm64),
        ("windows", "x86_64") => Ok(VscodeServerTarget::Win32X64),
        _ => bail!("unsupported host platform: {}-{}", env::consts::OS, env::consts::ARCH),
    }
}

fn vscode_target_server_path(workspace_root: &Path, target: VscodeServerTarget) -> PathBuf {
    workspace_root
        .join("editors")
        .join("vscode")
        .join("server")
        .join(target.folder())
        .join(target.binary_file())
}

fn server_build_args_for_vscode_target(
    target: VscodeServerTarget,
    profile: ExtensionBuildProfile,
    profile_trace: bool,
) -> ServerBuildArgs {
    ServerBuildArgs {
        profile,
        cargo_target: target.cargo_target().map(str::to_owned),
        alpine_linker: target.requires_alpine_linker(),
        profile_trace,
    }
}

fn cargo_build_args(args: &ServerBuildArgs) -> Vec<String> {
    let mut command_args = vec!["build".to_owned()];
    if args.profile == ExtensionBuildProfile::Release {
        command_args.push("--release".to_owned());
    }
    if let Some(cargo_target) = &args.cargo_target {
        command_args.push("--target".to_owned());
        command_args.push(cargo_target.clone());
    }
    if args.profile_trace {
        command_args.push("--features".to_owned());
        command_args.push("profile-trace".to_owned());
    }
    command_args
}

fn cargo_profile_dir(profile: ExtensionBuildProfile) -> &'static str {
    match profile {
        ExtensionBuildProfile::Debug => "debug",
        ExtensionBuildProfile::Release => "release",
    }
}

fn cargo_output_dir(workspace_root: &Path, args: &ServerBuildArgs) -> PathBuf {
    let mut path = workspace_root.join("target");
    if let Some(cargo_target) = &args.cargo_target {
        path = path.join(cargo_target);
    }
    path.join(cargo_profile_dir(args.profile))
}

fn server_binary_file(args: &ServerBuildArgs) -> &'static str {
    if args.cargo_target.as_deref().is_some_and(|target| target.contains("windows"))
        || (args.cargo_target.is_none() && cfg!(windows))
    {
        "vide.exe"
    } else {
        "vide"
    }
}

fn cargo_build_env_updates(args: &ServerBuildArgs) -> Vec<(String, String)> {
    let Some(cargo_target) = args.cargo_target.as_deref() else {
        return Vec::new();
    };

    let mut updates = Vec::new();
    let linker_env_key = cargo_target_linker_env_key(cargo_target);
    if optional_env(&linker_env_key).is_none()
        && let Some(linker) = cargo_linker_for_target(args, cargo_target)
    {
        eprintln!("Using Cargo linker for {cargo_target}: {linker}");
        updates.push((linker_env_key, linker));
    }

    let late_link_args = late_rust_link_flags_for_target(args);
    if !late_link_args.is_empty() {
        eprintln!("Adding Cargo link args for {cargo_target}: {}", late_link_args.join(" "));
        updates.push(rust_flags_env_update(&late_link_args));
    }

    updates
}

fn cargo_target_linker_env_key(cargo_target: &str) -> String {
    format!("CARGO_TARGET_{}_LINKER", cargo_target_env_name(cargo_target))
}

fn cargo_target_env_name(cargo_target: &str) -> String {
    cargo_target.to_uppercase().replace('-', "_")
}

fn cargo_linker_for_target(args: &ServerBuildArgs, cargo_target: &str) -> Option<String> {
    if !args.alpine_linker {
        return None;
    }

    optional_env(&cxx_compiler_env_key(cargo_target))
        .or_else(|| optional_env("TARGET_CXX"))
        .or_else(|| Some(format!("{cargo_target}-g++")))
}

fn cxx_compiler_env_key(cargo_target: &str) -> String {
    format!("CXX_{}", cargo_target.replace('-', "_"))
}

fn late_rust_link_flags_for_target(args: &ServerBuildArgs) -> Vec<&'static str> {
    if args.alpine_linker {
        // Static libstdc++ can introduce libc references after rustc's own musl -lc.
        vec!["-C", "link-arg=-lc"]
    } else {
        Vec::new()
    }
}

fn rust_flags_env_update(flags: &[&str]) -> (String, String) {
    if let Some(encoded_flags) = optional_env("CARGO_ENCODED_RUSTFLAGS") {
        return (
            "CARGO_ENCODED_RUSTFLAGS".to_owned(),
            format!("{encoded_flags}\x1f{}", flags.join("\x1f")),
        );
    }

    let rust_flags = optional_env("RUSTFLAGS");
    let flags = flags.join(" ");
    (
        "RUSTFLAGS".to_owned(),
        rust_flags.map_or(flags.clone(), |rust_flags| format!("{rust_flags} {flags}")),
    )
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name).ok().map(|value| value.trim().to_owned()).filter(|value| !value.is_empty())
}

fn ensure_vscode_server_executable(path: &Path, target: VscodeServerTarget) -> Result<()> {
    if target.is_windows() {
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }

    Ok(())
}

fn run_command(
    command: &str,
    args: &[String],
    cwd: &Path,
    env_updates: &[(String, String)],
) -> Result<()> {
    let mut child = ProcessCommand::new(command);
    child.args(args).current_dir(cwd);
    for (key, value) in env_updates {
        child.env(key, value);
    }

    let status = child
        .status()
        .with_context(|| format!("failed to run `{}` in {}", command, cwd.display()))?;

    if !status.success() {
        bail!("`{} {}` failed with {}", command, args.join(" "), status);
    }

    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest directory has no parent")
}

fn checked_in_manifest_schema_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(project_model::TOML_MANIFEST_SCHEMA_PATH.trim_start_matches('/'))
}

fn checked_in_user_config_schema_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(USER_CONFIG_SCHEMA_PATH.trim_start_matches('/'))
}

fn vscode_schema_constants_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(VSCODE_SCHEMA_CONSTANTS_PATH)
}

fn vscode_configuration_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(VSCODE_CONFIGURATION_PATH)
}

fn vscode_package_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(VSCODE_PACKAGE_PATH)
}

fn generated_manifest_schema() -> serde_json::Value {
    project_model::generated_toml_manifest_schema()
}

fn generated_manifest_schema_text() -> Result<String> {
    let generated = generated_manifest_schema();
    Ok(format!("{}\n", serde_json::to_string_pretty(&generated)?))
}

fn user_config_schema_text() -> Result<String> {
    let schema = vide::generated_user_config_schema();
    Ok(format!("{}\n", serde_json::to_string_pretty(&schema)?))
}

fn generated_vscode_schema_constants_text() -> Result<String> {
    let version = serde_json::to_string(project_model::TOML_MANIFEST_SCHEMA_VERSION)?;
    let path = serde_json::to_string(project_model::TOML_MANIFEST_SCHEMA_PATH)?;
    let url = serde_json::to_string(project_model::TOML_MANIFEST_SCHEMA_URL)?;

    Ok(format!(
        "// Generated by `cargo xtask generate-schemas`; do not edit.\n\
         export const PROJECT_CONFIG_SCHEMA_VERSION = {version};\n\
         export const PROJECT_CONFIG_SCHEMA_PATH = {path};\n\
         export const PROJECT_CONFIG_SCHEMA_URL = {url};\n"
    ))
}

fn generated_vscode_configuration_text() -> String {
    vide::generated_vscode_configuration_typescript()
}

fn generated_vscode_package_text(workspace_root: &Path) -> Result<String> {
    let path = vscode_package_path(workspace_root);
    let mut package = read_json_file(&path)?;
    patch_vscode_package_properties(&mut package)?;
    Ok(format!("{}\n", serde_json::to_string_pretty(&package)?))
}

fn patch_vscode_package_properties(package: &mut serde_json::Value) -> Result<()> {
    let Some(properties) = package
        .pointer_mut("/contributes/configuration/properties")
        .and_then(serde_json::Value::as_object_mut)
    else {
        bail!("editors/vscode/package.json has no contributes.configuration.properties object");
    };

    properties.retain(|key, _| !is_generated_vscode_setting(key));
    properties.extend(vide::generated_vscode_package_properties());
    Ok(())
}

fn is_generated_vscode_setting(key: &str) -> bool {
    key.starts_with("vide.") && !is_extension_only_vscode_setting(key)
}

fn is_extension_only_vscode_setting(key: &str) -> bool {
    matches!(
        key,
        "vide.trace.server"
            | "vide.server.command"
            | "vide.server.args"
            | "vide.server.cwd"
            | "vide.server.additionalArgs"
    )
}

fn write_config_artifacts(workspace_root: &Path) -> Result<()> {
    write_schemas(workspace_root)?;
    write_generated_file(
        &vscode_configuration_path(workspace_root),
        &generated_vscode_configuration_text(),
    )?;
    write_generated_file(
        &vscode_package_path(workspace_root),
        &generated_vscode_package_text(workspace_root)?,
    )?;
    Ok(())
}

fn check_config_artifacts(workspace_root: &Path) -> Result<()> {
    check_schemas(workspace_root)?;
    check_file_matches(
        &vscode_configuration_path(workspace_root),
        &generated_vscode_configuration_text(),
    )?;
    check_file_matches(
        &vscode_package_path(workspace_root),
        &generated_vscode_package_text(workspace_root)?,
    )?;
    Ok(())
}

fn write_schemas(workspace_root: &Path) -> Result<()> {
    let manifest_schema = generated_manifest_schema_text()?;
    let user_config_schema = user_config_schema_text()?;

    write_generated_file(&checked_in_manifest_schema_path(workspace_root), &manifest_schema)?;
    write_generated_file(&checked_in_user_config_schema_path(workspace_root), &user_config_schema)?;
    write_generated_file(
        &vscode_schema_constants_path(workspace_root),
        &generated_vscode_schema_constants_text()?,
    )?;
    Ok(())
}

fn check_schemas(workspace_root: &Path) -> Result<()> {
    let manifest_schema = generated_manifest_schema_text()?;
    let user_config_schema = user_config_schema_text()?;

    check_file_matches(&checked_in_manifest_schema_path(workspace_root), &manifest_schema)?;
    check_file_matches(&checked_in_user_config_schema_path(workspace_root), &user_config_schema)?;
    check_file_matches(
        &vscode_schema_constants_path(workspace_root),
        &generated_vscode_schema_constants_text()?,
    )?;

    Ok(())
}

fn write_generated_file(path: &Path, contents: &str) -> Result<()> {
    let Some(parent) = path.parent() else {
        bail!("generated file path has no parent: {}", path.display());
    };

    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

fn read_json_file(path: &Path) -> Result<serde_json::Value> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

fn check_file_matches(path: &Path, expected: &str) -> Result<()> {
    let checked_in =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    if checked_in != expected {
        bail!("{} is stale; run `cargo xtask generate-schemas`", path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::*;

    #[test]
    fn checked_in_schemas_match_generated_schemas() {
        check_schemas(&workspace_root().unwrap()).unwrap();
    }

    #[test]
    fn parses_vscode_prepare_server_command_with_clap() {
        let cli = Cli::try_parse_from([
            "xtask",
            "vscode",
            "prepare-server",
            "--target",
            "linux-x64",
            "--profile",
            "release",
            "--server",
            "prebuilt",
        ])
        .unwrap();

        let Some(XtaskCommand::Vscode(VscodeArgs { command: VscodeCommand::PrepareServer(args) })) =
            cli.command
        else {
            panic!("expected vscode prepare-server command");
        };

        assert_eq!(
            args,
            VscodePrepareServerArgs {
                target: VscodeServerTarget::LinuxX64,
                profile: ExtensionBuildProfile::Release,
                server: ExtensionServerMode::Prebuilt,
                profile_trace: false,
            }
        );
    }

    #[test]
    fn maps_alpine_vscode_target_to_server_build_args() {
        let args = server_build_args_for_vscode_target(
            VscodeServerTarget::AlpineX64,
            ExtensionBuildProfile::Release,
            true,
        );

        assert_eq!(
            args,
            ServerBuildArgs {
                profile: ExtensionBuildProfile::Release,
                cargo_target: Some("x86_64-unknown-linux-musl".to_owned()),
                alpine_linker: true,
                profile_trace: true,
            }
        );
        assert_eq!(
            cargo_build_args(&args),
            [
                "build",
                "--release",
                "--target",
                "x86_64-unknown-linux-musl",
                "--features",
                "profile-trace",
            ]
            .map(str::to_owned)
        );
        assert_eq!(server_binary_file(&args), "vide");

        let args = server_build_args_for_vscode_target(
            VscodeServerTarget::AlpineArm64,
            ExtensionBuildProfile::Release,
            false,
        );

        assert_eq!(
            args,
            ServerBuildArgs {
                profile: ExtensionBuildProfile::Release,
                cargo_target: Some("aarch64-unknown-linux-musl".to_owned()),
                alpine_linker: true,
                profile_trace: false,
            }
        );
    }
}
