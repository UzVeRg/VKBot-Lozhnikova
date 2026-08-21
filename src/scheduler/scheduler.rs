use crate::scheduler::models::{
    ScheduledMessage,
    WeeklyMessage,
};

use crate::scheduler::users::load_users;

use chrono::{
    DateTime,
    Datelike,
    Duration,
    FixedOffset,
    NaiveDate,
    NaiveDateTime,
    NaiveTime,
    Utc,
    Weekday,
};

use reqwest::Client;

use std::fs;
use std::time::Duration as StdDuration;
use crate::texts::get_message;
use tokio::time::sleep;


const VK_API_VERSION: &str = "5.199";

const SCHEDULE_FILE: &str =
    "data/scheduled_messages.json";

const WEEKLY_FILE: &str =
    "data/weekly_messages.json";


const WEEKLY_STATE_FILE: &str =
    "data/weekly_state.json";

// ============================================================
// Основной планировщик
// ============================================================

pub async fn start_scheduler(
    token: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    println!("Планировщик сообщений запущен.");

    loop {

        // Проверяем разовые сообщения.
        if let Err(error) =
            check_scheduled_messages(&token).await
        {
            eprintln!(
                "Ошибка разовых сообщений: {}",
                error
            );
        }

        // Проверяем, не наступил ли день регулярной рассылки.
        if let Err(error) =
            check_weekly_messages(&token).await
        {
            eprintln!(
                "Ошибка регулярной рассылки: {}",
                error
            );
        }

        // Проверяем расписание раз в 10 секунд.
        sleep(
            StdDuration::from_secs(10)
        ).await;
    }
}


// ============================================================
// РАЗОВЫЕ СООБЩЕНИЯ
// ============================================================

async fn check_scheduled_messages(
    token: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let content =
        fs::read_to_string(SCHEDULE_FILE)?;

    let messages: Vec<ScheduledMessage> =
        serde_json::from_str(&content)?;

    let now = Utc::now();

    for message in messages {

        if !message.enabled {
            continue;
        }

        let scheduled_time: DateTime<FixedOffset> =
            message.datetime.parse()?;

        let scheduled_time_utc =
            scheduled_time.with_timezone(&Utc);

        if scheduled_time_utc > now {
            continue;
        }

        println!(
            "Разовое сообщение: {}",
            message.id
        );

        send_to_all_users(
            token,
            &message.text,
        ).await?;
    }

    Ok(())
}


// ============================================================
// РЕГУЛЯРНАЯ РАССЫЛКА
// ============================================================

async fn check_weekly_messages(
    token: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let now = local_time();

    let weekday = now.weekday();

    // --------------------------------------------------------
    // Разрешённые дни
    // --------------------------------------------------------

    if weekday != Weekday::Wed
        && weekday != Weekday::Sat
    {
        return Ok(());
    }

    // --------------------------------------------------------
    // Время рассылки
    // --------------------------------------------------------

    let target_time =
        NaiveTime::from_hms_opt(18, 0, 0)
            .unwrap();

    if now.time() < target_time {
        return Ok(());
    }

    // --------------------------------------------------------
    // Определяем сегодняшний день
    // --------------------------------------------------------

    let today =
        now.date()
            .format("%Y-%m-%d")
            .to_string();

    let today_name = match weekday {

        Weekday::Wed => "wednesday",

        Weekday::Sat => "saturday",

        _ => return Ok(()),
    };

    // --------------------------------------------------------
    // Загружаем состояние
    // --------------------------------------------------------

    let state_content =
        fs::read_to_string(WEEKLY_STATE_FILE)?;

    let mut state:
        crate::scheduler::models::WeeklyState =
        serde_json::from_str(&state_content)?;

    // --------------------------------------------------------
    // Проверяем, была ли уже рассылка сегодня
    // --------------------------------------------------------

    let already_sent = match weekday {

        Weekday::Wed =>
            state.last_wednesday == today,

        Weekday::Sat =>
            state.last_saturday == today,

        _ => true,
    };

    if already_sent {
        return Ok(());
    }

    // --------------------------------------------------------
    // Загружаем регулярные сообщения
    // --------------------------------------------------------

    let messages_content =
        fs::read_to_string(WEEKLY_FILE)?;

    let messages:
        Vec<crate::scheduler::models::WeeklyMessage> =
        serde_json::from_str(
            &messages_content
        )?;

    // --------------------------------------------------------
    // Ищем сообщение именно для сегодняшнего дня
    // --------------------------------------------------------

    for message in messages {

        if !message.enabled {
            continue;
        }

        if message.weekday != today_name {
            continue;
        }

        println!(
            "Регулярная рассылка: {}",
            message.id
        );

        let text = get_message(
            &message.text
        )
        .ok_or_else(|| {
            format!(
                "Неизвестное сообщение: {}",
                message.text
            )
        })?;
        
        send_to_all_users(
            token,
            text,
        ).await?;
    }

    // --------------------------------------------------------
    // Запоминаем дату рассылки
    // --------------------------------------------------------

    match weekday {

        Weekday::Wed => {
            state.last_wednesday = today;
        }

        Weekday::Sat => {
            state.last_saturday = today;
        }

        _ => {}
    }

    let json =
        serde_json::to_string_pretty(&state)?;

    fs::write(
        WEEKLY_STATE_FILE,
        json,
    )?;

    Ok(())
}


// ============================================================
// ОТПРАВКА ВСЕМ СОХРАНЁННЫМ ПОЛЬЗОВАТЕЛЯМ
// ============================================================

async fn send_to_all_users(
    token: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let users = load_users()?;

    println!(
        "Получателей: {}",
        users.len()
    );

    let client = Client::new();

    for peer_id in users {

        println!(
            "Отправка пользователю {}",
            peer_id
        );

        let result = client
            .get(
                "https://api.vk.com/method/messages.send"
            )
            .query(&[
                ("access_token", token),
                ("v", VK_API_VERSION),
                (
                    "peer_id",
                    &peer_id.to_string()
                ),
                ("random_id", "0"),
                ("message", text),
            ])
            .send()
            .await?;

        if let Err(error) =
            result.error_for_status()
        {
            eprintln!(
                "Ошибка отправки пользователю {}: {}",
                peer_id,
                error
            );
        }

        sleep(
            StdDuration::from_millis(100)
        ).await;
    }

    println!("Рассылка завершена.");

    Ok(())
}


// ============================================================
// ТЕКУЩЕЕ ЛОКАЛЬНОЕ ВРЕМЯ
// ============================================================

fn local_time() -> NaiveDateTime {

    // UTC+3
    let offset =
        FixedOffset::east_opt(3 * 60 * 60)
            .unwrap();

    Utc::now()
        .with_timezone(&offset)
        .naive_local()
}