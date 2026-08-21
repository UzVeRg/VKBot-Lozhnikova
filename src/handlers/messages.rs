use vkoxide::prelude::*;
use crate::infrastructure::photo;
use crate::config::Config;
use crate::scheduler::users::add_user;
use crate::texts::*;

pub async fn greetings_handler(bot: Bot, message: MessageNewObject) -> HandlerResult {
    add_user(message.message.peer_id)?;
    let textex = message.message.text.trim();

    let user = bot.get_user(message.message.from_id).await?;
    
    let response = GREETING.replace(
        "{name}",
        &user.first_name,
    );

    bot.send_message(message.message.peer_id, &response, None)
        .await?;

    Ok(())
}

// то же самое, что и метод выше, но с фото
pub async fn greeting_and_photo_handler(
    bot: Bot,
    message: MessageNewObject,
) -> HandlerResult {
    add_user(message.message.peer_id)?;

    let config = Config::from_env()?;

    let peer_id = message.message.peer_id;

    let user = bot.get_user(message.message.from_id).await?;
    
    let response = GREETING.replace(
        "{name}",
        &user.first_name,
    );

    // Загружаем фотографию
    let attachment = photo::upload_photo(
        &config.vk_token,
        peer_id,
        "assets/hello.jpg",
    )
    .await?;

    // Отправляем фотографию вместе с текстом
    photo::send_message(
        &config.vk_token,
        peer_id,
        &response, // текст
        vec![attachment],
    )
    .await?;

    Ok(())
}



pub async fn any_text_message(bot: Bot, message: MessageNewObject) -> HandlerResult{
    add_user(message.message.peer_id)?;

    let response = ANY;
    
    bot.send_message(message.message.peer_id, &response, None)
        .await?;
    
    Ok(())
}
