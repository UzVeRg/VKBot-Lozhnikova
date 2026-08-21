



// ОСТАВЬ НАДЕЖДУ ВСЯК СЮДА ВХОДЯЩИЙ




mod bot;
mod config;
mod handlers;
mod infrastructure;
mod services;
mod texts;
mod scheduler;


use config::Config;
use infrastructure::longpoll::LongPollRunner;

use vkoxide::dptree;
use vkoxide::dptree::filter;
use vkoxide::filter;
use vkoxide::prelude::*;
use scheduler::scheduler::start_scheduler;
use handlers::commands::*;
use handlers::messages::*;



//use infrastructure::photo::send_photo;


fn schema() -> dptree::Handler<'static, DependencyMap, HandlerResult> {
    dptree::entry()
        .branch(filter::command("/start").endpoint(start_handler))
        .branch(filter::command("/help").endpoint(help_handler))
        .branch(filter::command("/info").endpoint(info_handler))
        .branch(filter::command("/random").endpoint(random_handler))



        // Мне стыдно за херню ниже, но после слов заказчицы мне стало в лом придумывать что-то для красоты и практичности
        .branch(filter::is_text("привет").endpoint(greeting_and_photo_handler))
        .branch(filter::is_text("Привет").endpoint(greeting_and_photo_handler))
        .branch(filter::is_text("ПРИВЕТ").endpoint(greeting_and_photo_handler))
        .branch(filter::is_text("Здравствуйте").endpoint(greeting_and_photo_handler))
        .branch(filter::is_text("здравствуйте").endpoint(greeting_and_photo_handler))
        .branch(filter::is_text("ЗДРАВСТВУЙТЕ").endpoint(greeting_and_photo_handler))


        .branch(filter::is_text("Начать").endpoint(greeting_and_photo_handler))
        .branch(filter::is_text("начать").endpoint(greeting_and_photo_handler))
        .branch(filter::is_text("НАЧАТЬ").endpoint(greeting_and_photo_handler))



        

        .branch(filter::is_text("Варианты").endpoint(variables_with_photo_handler))
        .branch(filter::is_text("варианты").endpoint(variables_with_photo_handler))
        .branch(filter::is_text("ВАРИАНТЫ").endpoint(variables_with_photo_handler))


        .branch(filter::is_text("ХОЧУ").endpoint(want_handle))
        .branch(filter::is_text("хочу").endpoint(want_handle))
        .branch(filter::is_text("Хочу").endpoint(want_handle))



        .branch(filter::is_text("Контакты").endpoint(contacts_handle))
        .branch(filter::is_text("контакты").endpoint(contacts_handle))
        .branch(filter::is_text("КОНТАКТЫ").endpoint(contacts_handle))
        
        
        

        .branch(filter::any_message().endpoint(any_text_message))

    
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv()?;

    tracing_subscriber::fmt::init();

    println!("VK-бот запускается...");

    let config = Config::from_env()?;

    
    let scheduler_token = config.vk_token.clone();
    
    tokio::spawn(async move {
    
        if let Err(error) =
            start_scheduler(scheduler_token).await
        {
            eprintln!(
                "Scheduler завершился с ошибкой: {}",
                error
            );
        }
    
    });

    let bot = bot::create_bot(&config);

    let runner = LongPollRunner::new(bot, schema());

    runner.run().await?;

    Ok(())
}
