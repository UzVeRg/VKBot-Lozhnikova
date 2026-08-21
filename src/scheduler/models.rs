use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledMessage {
    pub id: String,
    pub datetime: String,
    pub text: String,
    pub enabled: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyMessage {
    pub id: String,
    pub weekday: String,
    pub text: String,
    pub enabled: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyState {
    pub last_wednesday: String,
    pub last_saturday: String,
}