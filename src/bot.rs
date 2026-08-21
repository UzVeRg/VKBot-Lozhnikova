use vkoxide::Bot;

use crate::config::Config;

pub fn create_bot(config: &Config) -> Bot {
    Bot::new(config.vk_token.clone(), config.vk_group_id.clone())
}
