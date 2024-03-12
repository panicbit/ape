use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::buildbot;

pub fn guess_core_name_from_extension(extension: &str) -> Option<&'static str> {
    let extension = extension.to_ascii_lowercase();

    Some(match &*extension {
        "gb" | "gbc" => "gambatte",
        "gba" => "mgba",
        _ => return None,
    })
}

pub fn core_name_to_library_name(core_name: &str) -> String {
    format!("{core_name}_libretro.{}", std::env::consts::DLL_EXTENSION)
}

pub fn cores_directory() -> PathBuf {
    PathBuf::from("./cores/")
}

pub fn find_and_potentially_fetch_core_for_rom(rom: &Path) -> Result<PathBuf> {
    let extension = rom
        .extension()
        .context("rom has no extension: rename rom or explicitly specify a core")?
        .to_str()
        .context("rom extension is invalid utf-8")?;

    let core_name = guess_core_name_from_extension(extension)
        .with_context(|| format!("no core known to handle `{extension}` roms"))?;
    let library_name = core_name_to_library_name(core_name);
    let library_path = cores_directory().join(&library_name);

    if !library_path.exists() {
        println!("Downloading `{library_name}…`");

        buildbot::download_core_to_core_directory(core_name).context("failed to download core")?;

        println!("Download successful!");
    }

    Ok(library_path)
}
