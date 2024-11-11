use std::{
    io::{BufWriter, Write},
    path::Path,
    process::ExitCode,
};

use anyhow::Context;
use fxhash::FxHashMap;
use serde::{Deserialize, Serialize};
use toml::from_str;
use tracing::{span, Level};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default = "default_executable_name")]
    #[serde(alias = "executable-name")]
    pub executable_name: String,
    #[serde(default)]
    pub integrations: FxHashMap<String, String>,
    #[serde(default = "default_version")]
    pub version: semver::Version,
    #[serde(default = "default_teach_tech_core")]
    #[serde(alias = "teach-tech-core")]
    pub teach_tech_core: String,
}

fn default_executable_name() -> String {
    "teach-tech-built".to_string()
}

fn default_version() -> semver::Version {
    semver::Version::new(0, 1, 0)
}

fn default_teach_tech_core() -> String {
    "0.1.0".to_string()
}

pub fn build_at_path(path: &Path) -> anyhow::Result<ExitCode> {
    let BuildConfig {
        executable_name,
        integrations,
        version,
        teach_tech_core,
    } = from_str(
        &std::fs::read_to_string(path.join("build-config.toml"))
            .context("Reading build-config.toml")?,
    )
    .context("Parsing build-config.toml")?;
    let mut span = span!(Level::INFO, "Setting up {executable_name}");
    let mut _enter = span.enter();
    let executable_path = Path::new(&executable_name);

    if executable_path.exists() {
        if executable_path.is_file() {
            return Err(anyhow::anyhow!(
                "{executable_name} already exists and is a file"
            ));
        }
        if executable_path.join("src").exists() {
            if executable_path.join("src").is_file() {
                return Err(anyhow::anyhow!(
                    "{executable_name}/src already exists and is a file"
                ));
            }
        } else {
            std::fs::create_dir(executable_path.join("src"))
                .with_context(|| format!("Creating {executable_name}/src folder"))?;
        }
    } else {
        std::fs::create_dir(executable_path)
            .with_context(|| format!("Creating {executable_name} folder"))?;
        std::fs::create_dir(executable_path.join("src"))
            .with_context(|| format!("Creating {executable_name}/src folder"))?;
    }

    std::fs::write(executable_path.join(".gitignore"), "/target")
        .with_context(|| format!("Creating {executable_name}/.gitignore"))?;
    let file = std::fs::File::create(executable_path.join("Cargo.toml"))
        .with_context(|| format!("Creating {executable_name}/Cargo.toml"))?;
    let mut file = BufWriter::new(file);
    let write_result: std::io::Result<()> = try {
        writeln!(file, "[package]")?;
        writeln!(file, "name = \"{executable_name}\"")?;
        writeln!(file, "version = \"{version}\"")?;
        writeln!(file, "edition = \"2021\"")?;
        writeln!(file, "\n[dependencies]")?;

        if let Ok(version) = teach_tech_core.parse::<semver::Version>() {
            writeln!(file, "teach-tech-core = \"{version}\"")?;
        } else if Path::new(&teach_tech_core).is_absolute() {
            writeln!(file, "teach-tech-core.path = \"{teach_tech_core}\"")?;
        } else {
            writeln!(file, "teach-tech-core.path = \"../{teach_tech_core}\"")?;
        }
        writeln!(file, "anyhow = \"1.0.93\"")?;

        for (name, metadata) in &integrations {
            if let Ok(version) = metadata.parse::<semver::Version>() {
                writeln!(file, "{name} = \"{version}\"")?;
            } else if metadata.starts_with("http") {
                writeln!(file, "{name}.git = \"{metadata}\"")?;
            } else {
                let metadata_path = Path::new(&metadata);
                if !metadata_path.exists() {
                    return Err(anyhow::anyhow!("Path {metadata} does not exist"));
                }
                if !metadata_path.is_dir() {
                    return Err(anyhow::anyhow!("Path {metadata} is not a folder"));
                }
                if metadata_path.join("Cargo.toml").exists() {
                    if !metadata_path.join("Cargo.toml").is_file() {
                        return Err(anyhow::anyhow!("Path {metadata}/Cargo.toml is not a file"));
                    }
                } else {
                    return Err(anyhow::anyhow!("Path {metadata}/Cargo.toml does not exist"));
                }
                if metadata_path.join("src").exists() {
                    if !metadata_path.join("src").is_dir() {
                        return Err(anyhow::anyhow!("Path {metadata}/src is not a folder"));
                    }
                } else {
                    return Err(anyhow::anyhow!("Path {metadata}/src does not exist"));
                }
                if metadata_path.is_absolute() {
                    writeln!(file, "{name}.path = \"{metadata}\"")?;
                } else {
                    writeln!(file, "{name}.path = \"../{metadata}\"")?;
                }
            }
        }
    };
    write_result.with_context(|| format!("Writing to {executable_name}/Cargo.toml"))?;
    file.flush()
        .with_context(|| format!("Writing to {executable_name}/Cargo.toml"))?;
    drop(file);

    let file = std::fs::File::create(executable_path.join("src").join("main.rs"))
        .with_context(|| format!("Creating {executable_name}/src/main.rs"))?;
    let mut file = BufWriter::new(file);
    let write_result: std::io::Result<()> = try {
        writeln!(file, "use teach_tech_core::prelude::*;")?;
        writeln!(
            file,
            "\nfn main() -> anyhow::Result<std::process::ExitCode> {{"
        )?;
        writeln!(file, "\tinit_core(|mut core| async move {{")?;
        writeln!(file, "\t\tcore.add_info(\"version\", env!(\"CARGO_PKG_VERSION\"));")?;

        for (name, _) in &integrations {
            let name = name.replace("-", "_");
            // writeln!(file, "\t\tlet core = AddToCore::call({name}::add_to_core, core).await?;")?;
            writeln!(file, "\t\tlet core = {name}::add_to_core(core).await?;")?;
        }

        writeln!(file, "\t\tOk(core)")?;
        writeln!(file, "\t}})")?;
        writeln!(file, "}}")?;
    };
    write_result.with_context(|| format!("Writing to {executable_name}/Cargo.toml"))?;
    file.flush()
        .with_context(|| format!("Writing to {executable_name}/Cargo.toml"))?;
    drop(file);

    drop(_enter);
    span = span!(Level::INFO, "Building {executable_name}");
    _enter = span.enter();

    let status = std::process::Command::new("cargo")
        .arg("build")
        .current_dir(executable_path)
        .status()
        .with_context(|| format!("Building {executable_name}"))?;

    if status.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}
