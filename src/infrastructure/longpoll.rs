use std::time::{Duration, Instant};

use tokio::task::JoinHandle;
use vkoxide::prelude::*;


pub struct LongPollRunner {
    bot: Bot,
    handler: Handler<'static, DependencyMap, HandlerResult>,
}

impl LongPollRunner {
    pub fn new(bot: Bot, handler: Handler<'static, DependencyMap, HandlerResult>) -> Self {
        Self { bot, handler }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let mut consecutive_failures: u32 = 0;

        loop {
            println!("Подключение к VK Long Poll...");

            let started_at = Instant::now();

            let dispatcher = self.create_dispatcher();

            let dispatch_task = tokio::spawn(async move { dispatcher.dispatch().await });

            tokio::select! {
                result = dispatch_task => {
                    match result {
                        Ok(Ok(())) => {
                            println!("Dispatcher завершил работу.");
                            break;
                        }

                        Ok(Err(error)) => {
                            let uptime = started_at.elapsed();

                            println!(
                                "Long Poll завершился с ошибкой после {:.1} сек.: {}",
                                uptime.as_secs_f64(),
                                error
                            );

                            if uptime >= Duration::from_secs(60) {
                                consecutive_failures = 0;
                            }

                            consecutive_failures =
                                consecutive_failures.saturating_add(1);

                            let delay =
                                Self::reconnect_delay(consecutive_failures);

                            println!(
                                "Повторное подключение через {} сек.",
                                delay.as_secs()
                            );

                            tokio::time::sleep(delay).await;
                        }

                        Err(join_error) => {
                            let uptime = started_at.elapsed();

                            eprintln!(
                                "Dispatcher task аварийно завершился \
                                 после {:.1} сек.: {}",
                                uptime.as_secs_f64(),
                                join_error
                            );

                            consecutive_failures =
                                consecutive_failures.saturating_add(1);

                            let delay =
                                Self::reconnect_delay(consecutive_failures);

                            eprintln!(
                                "Повторное подключение через {} сек.",
                                delay.as_secs()
                            );

                            tokio::time::sleep(delay).await;
                        }
                    }
                }

                result = tokio::signal::ctrl_c() => {
                    match result {
                        Ok(()) => {
                            println!("\nПолучен сигнал завершения.");
                            println!("VK-бот останавливается...");
                        }

                        Err(error) => {
                            eprintln!(
                                "Не удалось получить Ctrl+C: {}",
                                error
                            );
                        }
                    }

                    break;
                }
            }
        }

        println!("VK-бот остановлен.");

        Ok(())
    }

    fn create_dispatcher(&self) -> Dispatcher {
        Dispatcher::builder(self.bot.clone())
            .handler(self.handler.clone())
            .build()
    }

    fn reconnect_delay(failures: u32) -> Duration {
        let seconds = match failures {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 4,
            4 => 8,
            5 => 16,
            _ => 30,
        };

        Duration::from_secs(seconds)
    }
}
