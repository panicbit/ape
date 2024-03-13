use std::env::consts::{ARCH, OS};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Context, Result};
use reqwest::Url;
use zip::ZipArchive;

fn buildbot_url_for_library(library_name: &str) -> Option<Url> {
    let mut url = Url::parse("https://buildbot.libretro.com/nightly").unwrap();
    let mut path = url.path_segments_mut().unwrap();

    match OS {
        "windows" => {
            path.push("windows");

            match ARCH {
                "x86" => path.push("x86"),
                "x86_64" => path.push("x86_64"),
                _ => return None,
            };
        }
        "linux" => {
            path.push("linux");

            match ARCH {
                "x86" => path.push("x86"),
                "x86_64" => path.push("x86_64"),
                _ => return None,
            };
        }
        "macos" => {
            path.extend(["apple", "osx"]);

            match ARCH {
                "x86" => path.push("x86"),
                "x86_64" => path.push("x86_64"),
                "arm64" => path.push("arm64"),
                _ => return None,
            };
        }
        _ => return None,
    }

    let zip_name = format!("{library_name}.zip");

    path.extend(["latest", &zip_name]);

    drop(path);

    Some(url)
}

fn download_library(library_name: &str) -> Result<Vec<u8>> {
    let url = buildbot_url_for_library(library_name)
        .context("Buildbot url for current platform is unknown")?;

    eprintln!("Downloading core from {url}");

    let response = reqwest::blocking::get(url)
        .and_then(|request| request.error_for_status())
        .context("Requesting core download failed")?;
    let zip = response.bytes()?;
    let zip = Cursor::new(zip);
    let mut zip = ZipArchive::new(zip)?;
    let mut file = zip.by_name(library_name).context("core not found in zip")?;
    let mut core = Vec::with_capacity(file.size() as usize);

    file.read_to_end(&mut core)?;

    Ok(core)
}

pub fn download_core_to(library_name: &str, path: &Path) -> Result<()> {
    let core = download_library(library_name)?;

    fs::write(path, core).with_context(|| format!("failed to write core to `{path:?}`"))?;

    Ok(())
}
