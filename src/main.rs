extern crate walkdir;
extern crate minimp3;
use walkdir::WalkDir;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use serde::Deserialize;
use audiotags::Tag;
use minimp3::{Decoder, Error as Mp3Error, Frame};
use umya_spreadsheet::*;

#[derive(Debug, Deserialize)]
struct Config {
    data_folder: String,
    extensions: Vec<String>,
    spreadsheet: SpreadsheetConfig
}

#[derive(Debug, Deserialize)]
struct SpreadsheetConfig {
    file_name: String,
    sheet: String,
    first_column: u32,
    first_row: u32
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

fn get_folder_bitrate_and_genre(extensions: &HashSet<&OsStr>, path: &Path, mut folder_bitrate: String, mut folder_genre: String) -> (String, String) {
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
                        let mut decoder = Decoder::new(file);

                        // Read the first frame to get the bitrate
                        match decoder.next_frame() {
                            Ok(Frame { bitrate, .. }) => {
                                let current_bitrate: String = bitrate.to_string();
                                if folder_bitrate.is_empty() {
                                    folder_bitrate = current_bitrate;
                                }
                                else if folder_bitrate != current_bitrate {
                                    folder_bitrate = String::from("VBR");
                                    break;
                                }
                            }
                            Err(Mp3Error::Eof) => {
                                eprintln!("File '{}' did not contain any frames!", file_path.to_string_lossy());
                            }
                            Err(e) => {
                                eprintln!("Cannot read file '{}'", file_path.to_string_lossy());
                                eprintln!("Error decoding: {:?}", e);
                            }
                        }
                    }
                    else {
                        if folder_bitrate.is_empty() {
                            folder_bitrate = file_extension;
                        }
                        else if folder_bitrate != file_extension {
                            folder_bitrate = String::from("?");
                        }
                    }

                    // Read tags
                    match Tag::new().read_from_path(file_path.clone()) {
                        Ok(tag) => {
                            // Successfully got the tag, use it here
                            let current_genre = match tag.genre() {
                                Some(current_genre) => current_genre.to_string().replace("\0", "/"),
                                None => String::from("?")
                            };
                            if folder_genre.is_empty() {
                                folder_genre = current_genre;
                            }
                            else if folder_genre != current_genre {
                                folder_genre = String::from("?");
                                break;
                            }
                        },
                        Err(e) => {
                            // Handle the error
                            eprintln!("Cannot read tag in file '{}'", file_path.to_string_lossy());
                            eprintln!("{}", e);
                            folder_genre = String::from("?");
                        }
                    }
                }
            }
        }
    }
    (folder_bitrate, folder_genre)
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

fn find_last_row_by_column_value(worksheet: &Worksheet, column_value: String, first_column: u32, first_row: u32) -> Option<u32> {
    // Iterate over the rows in reverse order, starting from the maximum row
    let max_row = worksheet.get_highest_row();
    for row in (first_row..=max_row).rev() {
        let value = worksheet.get_value((first_column, row));
        if value <= column_value {
            // Return the row index as an option
            return Some(row);
        }
    }
    None
}

fn main() {

    // Read the config file, parse and deserialize the config
    let config_str = fs::read_to_string("config.json").expect("ERROR: Failed to read config file");
    let config: Config = serde_json::from_str(&config_str).expect("ERROR: Failed to parse the config");

    let mut album_data: HashMap<String, Vec<(String, String, String, String, String)>> = HashMap::new();
    let mut total_albums: (u32, u32) = (0, 0);

    // Iterate over the folders
    for entry in WalkDir::new(config.data_folder).into_iter().filter_map(|entry: Result<walkdir::DirEntry, walkdir::Error>| entry.ok()) {
        if entry.metadata().map_or(false, |m| m.is_dir()) && entry.depth() == 2 {
            if let Some(name) = entry.file_name().to_str() {
                if let Some((year, name)) = name.split_once('-') {
                    if let Some(band_name) = get_band_name(&entry) {
                        let (bitrate, genre) = get_album_bitrate_and_genre(&config.extensions, entry.path());
                        album_data.entry(band_name.to_string()).or_insert_with(Vec::new).push(
                            (year.trim().to_string(), name.trim().to_string(), bitrate, genre, String::new()));
                        total_albums.0 += 1;
                    }
                }
            }
        } 
    }

    if total_albums.0 == 0 {
        println!("No albums have found!");
    }
    else {
        // Open the spreadsheet
        let path = std::path::Path::new(&(config.spreadsheet.file_name));
        let mut book = match reader::xlsx::read(path) {
            Ok(book) => book,
            Err(error) => panic!("Problem opening the file: {:?}", error),
        };
    
        let mut worksheet = match book.get_sheet_by_name_mut(&(config.spreadsheet.sheet)) {
            Ok(worksheet) => worksheet,
            Err(error) => panic!("Problem opening the worksheet: {:?}", error),
        };
    
        // Get cell styles from the top row
        let cell_styles: [Style; 6] = [
            worksheet.get_style((config.spreadsheet.first_column, config.spreadsheet.first_row)).clone(),
            worksheet.get_style((config.spreadsheet.first_column + 1, config.spreadsheet.first_row)).clone(),
            worksheet.get_style((config.spreadsheet.first_column + 2, config.spreadsheet.first_row)).clone(),
            worksheet.get_style((config.spreadsheet.first_column + 3, config.spreadsheet.first_row)).clone(),
            worksheet.get_style((config.spreadsheet.first_column + 4, config.spreadsheet.first_row)).clone(),
            worksheet.get_style((config.spreadsheet.first_column + 5, config.spreadsheet.first_row)).clone()
        ];
    
        // Iterate over the hashmap entries
        for (band, albums) in album_data {
            // Find the last row that contains the band name
            let last_row = find_last_row_by_column_value(worksheet, band.clone(), config.spreadsheet.first_column, config.spreadsheet.first_row).unwrap_or(worksheet.get_highest_row());
            // Insert new rows after the last row
            let rows: u32 = albums.len() as u32;
            worksheet.insert_new_row(&(last_row + 1), &rows);
            
            // Set the value of the new cells
            let mut row_index: u32 = last_row;
            for album in albums {
                // Destructure the tuple into an array
                let album_details: [String; 5] = [album.0, album.1, album.2, album.3, album.4];
        
                let mut coords = (config.spreadsheet.first_column, row_index + 1);
                // Fill band name
                worksheet.get_cell_mut(coords).set_value(band.to_string());
                //let mut style= ;
                worksheet.set_style(coords, cell_styles[0].clone().set_background_color("9A0F00").clone());
    
                // Fill album details
                for col_index in 0..4 {
                    coords = ((col_index + 2) as u32, row_index + 1);
                    worksheet.get_cell_mut(coords).set_value(album_details[col_index].to_string());
                    worksheet.set_style(coords, cell_styles[col_index + 1].clone().set_background_color("9A0F00").clone());
                }
    
                total_albums.1 += 1;
                row_index += 1;
            } 
        }
    
        let _ = match writer::xlsx::write(&book, path) {
            Ok(_) => println!("Successfully added {}/{} albums to the spreadsheet '{}'", total_albums.1, total_albums.0, config.spreadsheet.file_name),
            Err(error) => panic!("Problem saving changes: {:?}", error),
        };    
    }

}
