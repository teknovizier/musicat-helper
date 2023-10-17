extern crate walkdir;
use walkdir::WalkDir;
const DATA_DIRECTORY: &str = r#"C:\Users\User\Music\"#;

fn get_band_name(entry: &walkdir::DirEntry) -> Option<&str> {
    entry.path().parent()?.file_name()?.to_str()
}

fn main() {
    for entry in WalkDir::new(DATA_DIRECTORY).into_iter().filter_map(|entry: Result<walkdir::DirEntry, walkdir::Error>| entry.ok()) {
        if entry.metadata().map_or(false, |m| m.is_dir()) && entry.depth() == 2 {
            if let Some(name) = entry.file_name().to_str() {
                if let Some((album_year, album_name)) = name.split_once('-') {
                    if let Some(band_name) = get_band_name(&entry) {
                        println!("{};{};{}", band_name, album_year.trim(), album_name.trim());
                    }
                }
            }
        } 
    }
}
