use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::buildbot;

fn guess_core_name_from_extension(extension: &str) -> Option<&'static str> {
    let extension = extension.to_ascii_lowercase();

    Some(match &*extension {
        "gb" | "gbc" => "gambatte",
        "gba" => "mgba",
        _ => return None,
    })
}

fn core_name_to_library_name(core_name: &str) -> String {
    format!("{core_name}_libretro.{}", std::env::consts::DLL_EXTENSION)
}

// TODO: use `dirs` crate or similar
fn cores_directory() -> PathBuf {
    PathBuf::from("./cores/")
}

pub fn find_and_potentially_fetch_core_for_rom(rom: &Path) -> Result<PathBuf> {
    let core_manager = CoreManager::from_rom(rom)?;
    let library_path = core_manager.find_and_potentially_fetch()?;

    Ok(library_path.to_owned())
}

struct CoreManager {
    core_name: String,
    library_name: String,
    library_dir: PathBuf,
    library_path: PathBuf,
}

impl CoreManager {
    fn new(core_name: impl Into<String>) -> Self {
        let core_name = core_name.into();
        let library_name = core_name_to_library_name(&core_name);
        let library_dir = cores_directory();
        let library_path = library_dir.join(&library_name);

        Self {
            core_name,
            library_name,
            library_dir,
            library_path,
        }
    }

    fn from_rom(rom: &Path) -> Result<Self> {
        let extension = rom
            .extension()
            .context("rom has no extension: rename rom or explicitly specify a core")?
            .to_str()
            .context("rom extension is invalid utf-8")?;

        let core_name = guess_core_name_from_extension(extension)
            .with_context(|| format!("no core known to handle `{extension}` roms"))?;

        let this = Self::new(core_name);

        Ok(this)
    }

    fn find_and_potentially_fetch(&self) -> Result<&Path> {
        if !self.library_path.exists() {
            println!("Downloading `{}`…", self.library_name);

            fs::create_dir_all(&self.library_dir).context("failed to create core directory")?;

            buildbot::download_core_to(&self.library_name, &self.library_path)
                .context("failed to download core")?;

            println!("Download successful!");
        }

        Ok(&self.library_path)
    }
}
