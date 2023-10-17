extern crate walkdir;
use walkdir::WalkDir;
use std::fs;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    data_folder: String,
}

#[derive(Debug)]
struct AlbumInfo {
    band_name: String,
    album_year: String,
    album_name: String,
}

fn get_band_name(entry: &walkdir::DirEntry) -> Option<&str> {
    entry.path().parent()?.file_name()?.to_str()
}

fn main() {
    // Read the config file
    let config_str = fs::read_to_string("config.json").expect("ERROR: Failed to read config file");

    // Parse and deserialize the config
    let config: Config = serde_json::from_str(&config_str).expect("ERROR: Failed to parse the config");

    let mut album_data = Vec::new();

    for entry in WalkDir::new(config.data_folder).into_iter().filter_map(|entry: Result<walkdir::DirEntry, walkdir::Error>| entry.ok()) {
        if entry.metadata().map_or(false, |m| m.is_dir()) && entry.depth() == 2 {
            if let Some(name) = entry.file_name().to_str() {
                if let Some((album_year, album_name)) = name.split_once('-') {
                    if let Some(band_name) = get_band_name(&entry) {
                        album_data.push(AlbumInfo {
                            band_name: band_name.to_string(),
                            album_year: album_year.trim().to_string(),
                            album_name: album_name.trim().to_string(),
                        });
                    }
                }
            }
        } 
    }

    for album in &album_data {
        println!("{};{};{}", album.band_name, album.album_year, album.album_name);
    }
}
