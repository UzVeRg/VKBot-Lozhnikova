use std::fs;

const USERS_FILE: &str = "data/users.json";

pub fn load_users() -> Result<Vec<i64>, Box<dyn std::error::Error + Send + Sync>> {
    let content = fs::read_to_string(USERS_FILE)?;

    let users: Vec<i64> = serde_json::from_str(&content)?;

    Ok(users)
}

pub fn add_user(
    peer_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let mut users = load_users()?;

    if users.contains(&peer_id) {
        return Ok(());
    }

    users.push(peer_id);

    let json = serde_json::to_string_pretty(&users)?;

    fs::write(USERS_FILE, json)?;

    println!("Новый пользователь добавлен: {}", peer_id);

    Ok(())
}