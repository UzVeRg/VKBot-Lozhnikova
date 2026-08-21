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

const RETRY_DELAYS: [u64; 2] = [1, 2];

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

    let file_path = file_path.as_ref();


    // 1. Читаем файл


    let file_data = tokio::fs::read(file_path).await?;

    let file_name = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photo.jpg");

    println!(
        "Начинаем загрузку фотографии: {} ({} байт)",
        file_name,
        file_data.len()
    );


    // 2. Получаем upload URL + загружаем фото с retry


    let mut upload_result: Option<UploadResponse> = None;

    for attempt in 1..=MAX_UPLOAD_ATTEMPTS {
        println!(
            "Загрузка фотографии {} — попытка {}/{}",
            file_name,
            attempt,
            MAX_UPLOAD_ATTEMPTS
        );

        
        // Получаем НОВЫЙ upload_url на каждой попытке
        

        println!(
            "Получаем upload URL от VK (попытка {}/{})...",
            attempt,
            MAX_UPLOAD_ATTEMPTS
        );

        let upload_server_result = client
            .get("https://api.vk.com/method/photos.getMessagesUploadServer")
            .query(&[
                ("access_token", token),
                ("v", VK_API_VERSION),
                ("peer_id", &peer_id.to_string()),
            ])
            .send()
            .await;

        let upload_server_response = match upload_server_result {
            Ok(response) => {
                let status = response.status();

                if !status.is_success() {
                    let body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| {
                            "не удалось получить тело ответа".to_string()
                        });

                    println!(
                        "VK getMessagesUploadServer вернул HTTP {}: {}",
                        status,
                        body
                    );

                    if is_retryable_status(status)
                        && attempt < MAX_UPLOAD_ATTEMPTS
                    {
                        retry_delay(attempt).await;
                        continue;
                    }

                    return Err(
                        format!(
                            "Не удалось получить upload URL: HTTP {} — {}",
                            status,
                            body
                        )
                        .into(),
                    );
                }

                match response.json::<UploadServerResponse>().await {
                    Ok(data) => data,

                    Err(error) => {
                        println!(
                            "Ошибка разбора ответа getMessagesUploadServer: {}",
                            error
                        );

                        if attempt < MAX_UPLOAD_ATTEMPTS {
                            retry_delay(attempt).await;
                            continue;
                        }

                        return Err(
                            format!(
                                "VK вернул некорректный ответ getMessagesUploadServer: {}",
                                error
                            )
                            .into(),
                        );
                    }
                }
            }

            Err(error) => {
                println!(
                    "Сетевая ошибка при получении upload URL: {}",
                    error
                );

                if attempt < MAX_UPLOAD_ATTEMPTS {
                    retry_delay(attempt).await;
                    continue;
                }

                return Err(
                    format!(
                        "Не удалось получить upload URL после {} попыток: {}",
                        MAX_UPLOAD_ATTEMPTS,
                        error
                    )
                    .into(),
                );
            }
        };

        let upload_url = upload_server_response.response.upload_url;

        println!(
            "Получен upload URL. Начинаем отправку файла..."
        );

        
        // Создаём НОВЫЙ Part для каждой попытки
        

        let part = match multipart::Part::bytes(file_data.clone())
            .file_name(file_name.to_string())
            .mime_str("image/jpeg")
        {
            Ok(part) => part,

            Err(error) => {
                return Err(
                    format!(
                        "Не удалось создать multipart Part: {}",
                        error
                    )
                    .into(),
                );
            }
        };

        let form = multipart::Form::new()
            .part("photo", part);

        
        // Отправляем фотографию на upload server
        

        let response = client
            .post(&upload_url)
            .multipart(form)
            .send()
            .await;

        match response {
            Ok(response) => {
                let status = response.status();

                println!(
                    "VK upload server ответил HTTP {}",
                    status
                );

                // ------------------------------------------------
                // Успешная загрузка
                // ------------------------------------------------

                if status.is_success() {
                    match response.json::<UploadResponse>().await {
                        Ok(upload) => {
                            println!(
                                "Файл успешно загружен на VK upload server."
                            );

                            upload_result = Some(upload);
                            break;
                        }

                        Err(error) => {
                            println!(
                                "Ошибка разбора ответа upload server: {}",
                                error
                            );

                            if attempt < MAX_UPLOAD_ATTEMPTS {
                                retry_delay(attempt).await;
                                continue;
                            }

                            return Err(
                                format!(
                                    "VK вернул некорректный ответ при загрузке фотографии: {}",
                                    error
                                )
                                .into(),
                            );
                        }
                    }
                }

                // ------------------------------------------------
                // Временные ошибки 502 / 503 / 504
                // ------------------------------------------------

                if is_retryable_status(status) {
                    let body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| {
                            "не удалось получить тело ответа".to_string()
                        });

                    println!(
                        "Временная ошибка VK upload server: HTTP {} — {}",
                        status,
                        body
                    );

                    if attempt < MAX_UPLOAD_ATTEMPTS {
                        retry_delay(attempt).await;
                        continue;
                    }

                    return Err(
                        format!(
                            "VK upload server недоступен после {} попыток. Последний ответ: HTTP {} — {}",
                            MAX_UPLOAD_ATTEMPTS,
                            status,
                            body
                        )
                        .into(),
                    );
                }

                // ------------------------------------------------
                // Остальные HTTP ошибки
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
                    .into(),
                );
            }

            
            // Сетевая ошибка
            

            Err(error) => {
                println!(
                    "Ошибка соединения с VK upload server: {}",
                    error
                );

                if attempt < MAX_UPLOAD_ATTEMPTS {
                    retry_delay(attempt).await;
                    continue;
                }

                return Err(
                    format!(
                        "Не удалось загрузить фотографию после {} попыток: {}",
                        MAX_UPLOAD_ATTEMPTS,
                        error
                    )
                    .into(),
                );
            }
        }
    }


    // 3. Проверяем результат upload


    let upload = upload_result.ok_or(
        "VK не вернул результат загрузки фотографии",
    )?;

    println!(
        "Получены данные загрузки: server={}, photo={}, hash={}",
        upload.server,
        upload.photo,
        upload.hash
    );


    // 4. Сохраняем фото через VK API


    println!(
        "Сохраняем фотографию через photos.saveMessagesPhoto..."
    );

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


    // 5. Получаем сохранённую фотографию


    let photo = saved
        .response
        .first()
        .ok_or("VK не вернул сохранённую фотографию")?;

    println!(
        "Фотография сохранена: owner_id={}, id={}",
        photo.owner_id,
        photo.id
    );


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

    println!(
        "Фотография {} успешно обработана. Attachment: {}",
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


    // Формируем attachment


    let attachment = attachments.join(",");

    println!(
        "Отправляем сообщение в peer_id={}. Attachment: {}",
        peer_id,
        if attachment.is_empty() {
            "<нет>".to_string()
        } else {
            attachment.clone()
        }
    );


    // Отправляем сообщение


    let response = client
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
        .await?;

    let status = response.status();

    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| {
                "не удалось получить тело ответа".to_string()
            });

        println!(
            "VK messages.send вернул HTTP {}: {}",
            status,
            body
        );

        return Err(
            format!(
                "Ошибка VK messages.send: HTTP {} — {}",
                status,
                body
            )
            .into(),
        );
    }

    println!(
        "Сообщение успешно отправлено в peer_id={}",
        peer_id
    );

    Ok(())
}

// ============================================================
// RETRY HELPERS
// ============================================================

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::BAD_GATEWAY
        || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
        || status == reqwest::StatusCode::GATEWAY_TIMEOUT
}

async fn retry_delay(attempt: usize) {
    let delay = RETRY_DELAYS
        .get(attempt - 1)
        .copied()
        .unwrap_or(2);

    println!(
        "Повторная попытка через {} сек.",
        delay
    );

    sleep(Duration::from_secs(delay)).await;
}