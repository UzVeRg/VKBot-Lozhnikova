use reqwest::multipart;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;

const VK_API_VERSION: &str = "5.199";

// ============================================================
// НАСТРОЙКИ RETRY
// ============================================================

const MAX_UPLOAD_ATTEMPTS: usize = 3;

const RETRY_DELAYS: [u64; 2] = [
    1,
    2,
];

const HTTP_TIMEOUT_SECONDS: u64 = 30;

// ============================================================
// VK API STRUCTURES
// ============================================================

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

// ============================================================
// UPLOAD PHOTO
// ============================================================

pub async fn upload_photo(
    token: &str,
    peer_id: i64,
    file_path: impl AsRef<Path>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .build()?;

    // --------------------------------------------------------
    // 1. Получаем upload URL
    // --------------------------------------------------------

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

    // --------------------------------------------------------
    // 2. Читаем файл
    // --------------------------------------------------------

    let file_path = file_path.as_ref();

    let file_data = tokio::fs::read(file_path).await?;

    let file_name = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photo.jpg");

    // --------------------------------------------------------
    // 3. Создаём multipart
    // --------------------------------------------------------
    /* 
    let part = multipart::Part::bytes(file_data)
        .file_name(file_name.to_string())
        .mime_str("image/jpeg")?;
    */
    // --------------------------------------------------------
    // 4. Загружаем фотографию с retry
    // --------------------------------------------------------

    let mut upload_result: Option<UploadResponse> = None;

    for attempt in 1..=MAX_UPLOAD_ATTEMPTS {

        let form = multipart::Form::new()
            .part("photo", multipart::Part::bytes(file_data.clone())
                .file_name(file_name.to_string())
                .mime_str("image/jpeg")?);

        println!(
            "Загрузка фотографии {} (попытка {}/{})...",
            file_name,
            attempt,
            MAX_UPLOAD_ATTEMPTS
        );

        let response = client
            .post(&upload_server.response.upload_url)
            .multipart(form)
            .send()
            .await;

        match response {

            Ok(response) => {

                let status = response.status();

                // ------------------------------------------------
                // Успешная загрузка
                // ------------------------------------------------

                if status.is_success() {

                    match response.json::<UploadResponse>().await {

                        Ok(upload) => {
                            upload_result = Some(upload);
                            break;
                        }

                        Err(error) => {

                            if attempt == MAX_UPLOAD_ATTEMPTS {
                                return Err(
                                    format!(
                                        "VK вернул некорректный ответ при загрузке фотографии: {}",
                                        error
                                    )
                                    .into()
                                );
                            }

                            println!(
                                "Ошибка разбора ответа VK: {}. Повторная попытка...",
                                error
                            );
                        }
                    }

                    continue;
                }

                // ------------------------------------------------
                // Временные ошибки VK
                // ------------------------------------------------

                if status == reqwest::StatusCode::BAD_GATEWAY
                    || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
                    || status == reqwest::StatusCode::GATEWAY_TIMEOUT
                {

                    println!(
                        "VK upload server вернул HTTP {}.",
                        status
                    );

                    if attempt < MAX_UPLOAD_ATTEMPTS {

                        let delay =
                            RETRY_DELAYS[attempt - 1];

                        println!(
                            "Повторная попытка через {} сек.",
                            delay
                        );

                        sleep(
                            Duration::from_secs(delay)
                        ).await;

                        continue;
                    }

                    return Err(
                        format!(
                            "VK upload server недоступен после {} попыток. HTTP {}",
                            MAX_UPLOAD_ATTEMPTS,
                            status
                        )
                        .into()
                    );
                }

                // ------------------------------------------------
                // Другие HTTP ошибки
                // ------------------------------------------------

                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| {
                        "не удалось получить тело ответа".to_string()
                    });

                return Err(
                    format!(
                        "Ошибка VK upload server: HTTP {} — {}",
                        status,
                        body
                    )
                    .into()
                );
            }

            // ----------------------------------------------------
            // Сетевая ошибка
            // ----------------------------------------------------

            Err(error) => {

                println!(
                    "Ошибка соединения с VK upload server: {}",
                    error
                );

                if attempt < MAX_UPLOAD_ATTEMPTS {

                    let delay =
                        RETRY_DELAYS[attempt - 1];

                    println!(
                        "Повторная попытка через {} сек.",
                        delay
                    );

                    sleep(
                        Duration::from_secs(delay)
                    ).await;

                    continue;
                }

                return Err(
                    format!(
                        "Не удалось загрузить фотографию после {} попыток: {}",
                        MAX_UPLOAD_ATTEMPTS,
                        error
                    )
                    .into()
                );
            }
        }
    }

    // --------------------------------------------------------
    // Проверяем результат upload
    // --------------------------------------------------------

    let upload = upload_result.ok_or(
        "VK не вернул результат загрузки фотографии"
    )?;

    // --------------------------------------------------------
    // 5. Сохраняем фото через VK API
    // --------------------------------------------------------

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

    // --------------------------------------------------------
    // 6. Получаем сохранённую фотографию
    // --------------------------------------------------------

    let photo = saved
        .response
        .first()
        .ok_or("VK не вернул сохранённую фотографию")?;

    // --------------------------------------------------------
    // 7. Формируем attachment
    // --------------------------------------------------------

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

    println!(
        "Фотография {} успешно загружена: {}",
        file_name,
        attachment
    );

    Ok(attachment)
}

// ============================================================
// SEND MESSAGE
// ============================================================

pub async fn send_message(
    token: &str,
    peer_id: i64,
    message: &str,
    attachments: Vec<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .build()?;

    // --------------------------------------------------------
    // Формируем attachment
    // --------------------------------------------------------

    let attachment = attachments.join(",");

    // --------------------------------------------------------
    // Отправляем сообщение
    // --------------------------------------------------------

    client
        .get("https://api.vk.com/method/messages.send")
        .query(&[
            ("access_token", token),
            ("v", VK_API_VERSION),
            ("peer_id", &peer_id.to_string()),
            ("random_id", "0"),
            ("message", message),
            ("attachment", &attachment),
        ])
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}