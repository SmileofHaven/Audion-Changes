// Audio save and metadata commands
use lofty::{Accessor, MimeType, Picture, PictureType, Probe, TagExt, TaggedFileExt};
use std::fs;
use std::io::Write;
use std::path::Path;
use tauri::command;

#[derive(serde::Deserialize)]
pub struct DownloadAudioInput {
    pub url: String,
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub cover_url: Option<String>,
}

#[command]
pub async fn download_and_save_audio(input: DownloadAudioInput) -> Result<String, String> {
    let path = Path::new(&input.path);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Download the audio file from URL
    let audio_data = download_file(&input.url).await?;

    // Write to file
    let mut file = fs::File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;
    file.write_all(&audio_data)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    drop(file);

    // Try to write metadata (non-fatal if it fails)
    // AAC files with ID3 tags often fail to play in browsers, so we skip metadata for them
    let is_aac = path
        .extension()
        .map_or(false, |ext| ext.eq_ignore_ascii_case("aac"));
    if !is_aac {
        if let Err(e) = write_metadata_to_file(path, &input).await {
            eprintln!("Warning: Could not write metadata: {}", e);
        }
    }

    Ok(input.path)
}

async fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to download audio: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed with status: {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read audio: {}", e))?;

    Ok(bytes.to_vec())
}

async fn write_metadata_to_file(path: &Path, input: &DownloadAudioInput) -> Result<(), String> {
    // Read the file for metadata
    let mut tagged_file = Probe::open(path)
        .map_err(|e| format!("Failed to open file for metadata: {}", e))?
        .read()
        .map_err(|e| format!("Failed to read file for metadata: {}", e))?;

    // Get or create primary tag
    let tag = match tagged_file.primary_tag_mut() {
        Some(tag) => tag,
        None => {
            let tag_type = tagged_file.primary_tag_type();
            tagged_file.insert_tag(lofty::Tag::new(tag_type));
            tagged_file
                .primary_tag_mut()
                .ok_or("Failed to create tag")?
        }
    };

    // Set metadata
    if let Some(title) = &input.title {
        tag.set_title(title.clone());
    }
    if let Some(artist) = &input.artist {
        tag.set_artist(artist.clone());
    }
    if let Some(album) = &input.album {
        tag.set_album(album.clone());
    }
    if let Some(track_num) = input.track_number {
        tag.set_track(track_num);
    }

    // Download and set cover art if URL provided
    if let Some(cover_url) = &input.cover_url {
        if !cover_url.is_empty() {
            match download_cover(cover_url).await {
                Ok(cover_data) => {
                    let picture = Picture::new_unchecked(
                        PictureType::CoverFront,
                        Some(MimeType::Jpeg),
                        None,
                        cover_data,
                    );
                    tag.push_picture(picture);
                }
                Err(e) => {
                    eprintln!("Failed to download cover: {}", e);
                }
            }
        }
    }

    // Save the metadata
    tag.save_to_path(path)
        .map_err(|e| format!("Failed to save metadata: {}", e))?;

    Ok(())
}

async fn download_cover(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to fetch cover: {}", e))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read cover: {}", e))?;

    Ok(bytes.to_vec())
}
