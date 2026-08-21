pub struct PhotoService {
    api: VkApiClient,
}

impl PhotoService {

    pub async fn send_photo(
        &self,
        peer_id: i64,
        message: &str,
        path: &str,
    ) -> Result<(), VkApiError> {

        // 1. Получаем URL для загрузки

        // 2. Загружаем файл

        // 3. Сохраняем фотографию в VK

        // 4. Получаем attachment

        // 5. Отправляем messages.send

        Ok(())
    }
}