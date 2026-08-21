use std::env;

use thiserror::Error;

#[derive(Debug)]
pub struct Config {
    pub vk_token: String,
    pub vk_group_id: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let vk_token = env::var("VK_TOKEN").map_err(|_| ConfigError::Missing("VK_TOKEN"))?;

        let vk_group_id =
            env::var("VK_GROUP_ID").map_err(|_| ConfigError::Missing("VK_GROUP_ID"))?;

        Ok(Self {
            vk_token,
            vk_group_id,
        })
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("не найдена переменная окружения: {0}")]
    Missing(&'static str),
}
