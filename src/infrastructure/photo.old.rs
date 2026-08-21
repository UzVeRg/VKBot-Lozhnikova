use reqwest::multipart;
use serde::Deserialize;
use std::path::Path;

const VK_API_VERSION: &str = "5.199";

#[derive(Debug, Deserialize)]
struct UploadServerResponse {
    response: UploadServer,
}

#[derive(Debug, Deserialize)]
struct UploadServer {
    upload_url: String,
}

#[derive(Debug, Deserialize)]
struct UploadResponse {
    server: i64,
    photo: String,
    hash: String,
}

#[derive(Debug, Deserialize)]
struct SavePhotoResponse {
    response: Vec<SavedPhoto>,
}

#[derive(Debug, Deserialize)]
struct SavedPhoto {
    id: i64,
    owner_id: i64,

    #[serde(default)]
    access_key: Option<String>,
}

pub async fn upload_photo(
    token: &str,
    peer_id: i64,
    file_path: impl AsRef<Path>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {

    let client = reqwest::Client::new();

    // 1. Получаем upload URL

    let upload_server: UploadServerResponse = client
        .get("https://api.vk.com/method/photos.getMessagesUploadServer")
        .query(&[
            ("access_token", token),
            ("v", VK_API_VERSION),
            ("peer_id", &peer_id.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // 2. Читаем файл

    let file_path = file_path.as_ref();

    let file_data = tokio::fs::read(file_path).await?;

    let file_name = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photo.jpg");

    // 3. Multipart

    let part = multipart::Part::bytes(file_data)
        .file_name(file_name.to_string())
        .mime_str("image/jpeg")?;

    let form = multipart::Form::new()
        .part("photo", part);

    // 4. Загружаем файл

    let upload: UploadResponse = client
        .post(&upload_server.response.upload_url)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // 5. Сохраняем фото

    let saved: SavePhotoResponse = client
        .get("https://api.vk.com/method/photos.saveMessagesPhoto")
        .query(&[
            ("access_token", token),
            ("v", VK_API_VERSION),
            ("server", &upload.server.to_string()),
            ("photo", &upload.photo),
            ("hash", &upload.hash),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let photo = saved
        .response
        .first()
        .ok_or("VK не вернул сохранённую фотографию")?;

    // 6. Формируем attachment

    let attachment = match &photo.access_key {
        Some(access_key) => {
            format!(
                "photo{}_{}_{}",
                photo.owner_id,
                photo.id,
                access_key
            )
        }

        None => {
            format!(
                "photo{}_{}",
                photo.owner_id,
                photo.id
            )
        }
    };

    Ok(attachment)
}


pub async fn send_photo(
    token: &str,
    peer_id: i64,
    message: &str,
    attachment: Vec<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let client = reqwest::Client::new();
    let attachment_str = attachment.join(",");

    client
        .get("https://api.vk.com/method/messages.send")
        .query(&[
            ("access_token", token),
            ("v", VK_API_VERSION),
            ("peer_id", &peer_id.to_string()),
            ("random_id", "0"),
            ("message", message),
            ("attachment", &attachment_str),
        ])
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}