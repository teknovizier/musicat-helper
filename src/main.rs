extern crate walkdir;
extern crate puremp3;
use walkdir::WalkDir;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use puremp3::Mp3Decoder;
use serde::Deserialize;
use audiotags::Tag;

#[derive(Debug, Deserialize)]
struct Config {
    data_folder: String,
    extensions: Vec<String>
}

#[derive(Debug)]
struct AlbumInfo {
    band_name: String,
    year: String,
    name: String,
    bitrate: String,
    genre: String,
}

fn get_band_name(entry: &walkdir::DirEntry) -> Option<&str> {
    entry.path().parent()?.file_name()?.to_str()
}

fn get_subdirectories(path: &Path) -> Vec<PathBuf> {
    let mut subdirectories = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                subdirectories.push(entry.path());
            }
        }
    }
    subdirectories
}

fn get_folder_bitrate_and_genre(extensions: &HashSet<&OsStr>, path: &Path, mut bitrate: String, mut genre: String) -> (String, String) {
    // Read the directory
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_path = entry.path();
            if file_path.is_file() && file_path.extension().map_or(false, |e| extensions.contains(e)) {
                if let Some(ext) = file_path.extension() {
                    let file_extension = ext.to_string_lossy().to_uppercase();

                    // Read MP3 bitrate
                    if file_extension == "MP3" {
                        let file = File::open(file_path.clone()).unwrap();
                        let mut decoder = Mp3Decoder::new(file);

                        match decoder.next_frame() {
                            Ok(frame) => {
                                let current_bitrate = (frame.header.bitrate.bps() / 1000).to_string();
                                if bitrate.is_empty() {
                                    bitrate = current_bitrate;
                                }
                                else if bitrate != current_bitrate {
                                    bitrate = String::from("VBR");
                                    break;
                                }
                            }
                            Err(e) => {
                                // Handle the error
                                eprintln!("Cannot read file '{}'!", file_path.clone().to_string_lossy());
                                eprintln!("{}", e);
                                bitrate = String::from("?");
                            }
                        }
                    }
                    else {
                        if bitrate.is_empty() {
                            bitrate = file_extension;
                        }
                        else if bitrate != file_extension {
                            bitrate = String::from("?");
                        }
                    }

                    // Read tags
                    match Tag::new().read_from_path(file_path.clone()) {
                        Ok(tag) => {
                            // Successfully got the tag, use it here
                            let current_genre: String = tag.genre().unwrap().to_string();
                            if genre.is_empty() {
                                genre = current_genre;
                            }
                            else if genre != current_genre {
                                genre = String::from("?");
                                break;
                            }
                        },
                        Err(e) => {
                            // Handle the error
                            eprintln!("Cannot read tag in file '{}'!", file_path.to_string_lossy());
                            eprintln!("{}", e);
                            genre = String::from("?");
                        }
                    }
                }
            }
        }
    }
    (bitrate, genre)
}

fn get_album_bitrate_and_genre(extensions: &Vec<String>, path: &Path) -> (String, String) {
    let mut bitrate: String = String::new();
    let mut genre: String = String::new();

    // Set of allowed extensions
    let mut allowed_extensions: HashSet<&OsStr> = HashSet::new();
    for ext in extensions.iter() {
        allowed_extensions.insert(OsStr::new(ext));
    }

    let subdirectories = get_subdirectories(path);
    if subdirectories.is_empty() {
        (bitrate, genre) = get_folder_bitrate_and_genre(&allowed_extensions, path, bitrate, genre);
    }
    else {
        for entry in subdirectories {
            (bitrate, genre) = get_folder_bitrate_and_genre(&allowed_extensions, entry.as_path(), bitrate, genre);
        }
    }
    (bitrate, genre)
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
                if let Some((year, name)) = name.split_once('-') {
                    if let Some(band_name) = get_band_name(&entry) {
                        let (bitrate, genre) = get_album_bitrate_and_genre(&config.extensions, entry.path());
                        album_data.push(AlbumInfo {
                            band_name: band_name.to_string(),
                            year: year.trim().to_string(),
                            name: name.trim().to_string(),
                            bitrate,
                            genre
                        });
                    }
                }
            }
        } 
    }

    for album in &album_data {
        println!("{};{};{};{};{}", album.band_name, album.year, album.name, album.bitrate, album.genre);
    }
}
