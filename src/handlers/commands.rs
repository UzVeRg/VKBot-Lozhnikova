use std::vec;

use vkoxide::prelude::*;
use rand::Rng;
use crate::texts::*;
use crate::infrastructure::photo;
use crate::config::Config;
use crate::scheduler::users::add_user;

pub async fn start_handler(bot: Bot, message: MessageNewObject) -> HandlerResult {
    add_user(message.message.peer_id)?;
    bot.send_message(message.message.peer_id, "Привет! Я бот сообщества!\nПопробуй команду /help или просто скажи 'Привет'", 
        None,
    )
        .await?;

    Ok(())
}

pub async fn help_handler(bot: Bot, message: MessageNewObject) -> HandlerResult {
    add_user(message.message.peer_id)?;
    bot.send_message(
        message.message.peer_id,
        "Доступные команды:\n/random — случайное число от 0 до 99\n\n\nЧтобы получить всю необходимую информацию обо мне или проекте - достаточо просто поздороваться, сказав 'Привет'",
        None,
    )
    .await?;

    Ok(())
}

pub async fn info_handler(bot: Bot, message: MessageNewObject) -> HandlerResult {
    add_user(message.message.peer_id)?;
    bot.send_message(
        message.message.peer_id,
        "Если нужно помощь, то просто напиши 'Привет'",
        None,
    )
    .await?;

    Ok(())
}

pub async fn random_handler(bot: Bot,message: MessageNewObject,) -> HandlerResult {
    let random_number: u8 = rand::thread_rng().gen_range(0..100);
    let random_number_str = format!("Случайное число от 0 до 99:   {}",random_number);
    add_user(message.message.peer_id)?;

    bot.send_message(
        message.message.peer_id,
        &random_number_str,
        None,
    )
    .await?;

    Ok(())
}


pub async fn variables_handler(bot: Bot, message: MessageNewObject) -> HandlerResult{
    add_user(message.message.peer_id)?;

    let response = VAR;
    
    bot.send_message(message.message.peer_id, &response, None)
        .await?;
    
    Ok(())
}

pub async fn variables_with_photo_handler(bot: Bot, message: MessageNewObject) -> HandlerResult {
    add_user(message.message.peer_id)?;
    let config = Config::from_env()?;
    let response = VAR;
    
    let peer_id = message.message.peer_id;

    // Загружаем фотографию
    let attachment1 = photo::upload_photo(
        &config.vk_token,
        peer_id,
        "assets/var1.jpg",
    )
    .await?;

    let attachment2 = photo::upload_photo(
        &config.vk_token,
        peer_id,
        "assets/var2.jpg",
    )
    .await?;

    // Отправляем фотографию вместе с текстом
    photo::send_message(
        &config.vk_token,
        peer_id,
        &response, // текст
        vec![attachment1, attachment2],
    )
    .await?;
    
    Ok(())
}


pub async fn want_handle(
    bot: Bot,
    message: MessageNewObject,
) -> HandlerResult {
    add_user(message.message.peer_id)?;

    let config = Config::from_env()?;

    let peer_id = message.message.peer_id;

    let user = bot.get_user(message.message.from_id).await?;
    
    let response = WANT.replace(
        "{name}",
        &user.first_name,
    );

    // Загружаем фотографию
    let attachment = photo::upload_photo(
        &config.vk_token,
        peer_id,
        "assets/want.jpg",
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


pub async fn contacts_handle(bot: Bot, message: MessageNewObject) -> HandlerResult{
    add_user(message.message.peer_id)?;
    let response = CONT;
    
    bot.send_message(message.message.peer_id, &response, None)
        .await?;
    
    Ok(())
}