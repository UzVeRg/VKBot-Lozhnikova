#[path = "../infrastructure/photo.rs"]
mod photo;

use rand::random;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::sleep;

const VK_API_VERSION: &str = "5.199";
const SEND_DELAY_MS: u64 = 1100;

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Deserialize)]
struct CampaignConfig {
    photos: Vec<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct BroadcastState {
    #[serde(default)]
    recipients: Vec<i64>,
    sent: BTreeSet<i64>,
    failed: BTreeMap<i64, String>,
}

struct Campaign {
    id: String,
    message: String,
    photos: Vec<PathBuf>,
}

struct LockGuard(PathBuf);

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn load_campaign(id: &str) -> AppResult<Campaign> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Некорректный тип символов в сообщении".into());
    }

    let dir = Path::new("mailings").join(id);
    let message = fs::read_to_string(dir.join("message.txt"))?;
    if message.trim().is_empty() {
        return Err("message.txt пустое".into());
    }

    let config: CampaignConfig = serde_json::from_slice(&fs::read(dir.join("config.json"))?)?;
    if config.photos.len() > 10 {
        return Err("Максимум 10 фото в одном сообщении".into());
    }

    let mut photos = Vec::new();
    for name in config.photos {
        if Path::new(&name).components().count() != 1 {
            return Err(format!("Некорректное имя изображения: {name}").into());
        }
        let path = dir.join(&name);
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_ascii_lowercase();
        if ext != "jpg" && ext != "jpeg" {
            return Err(format!("Только .jpg и .jpeg поддерживаются: {name}").into());
        }
        if !path.is_file() || fs::metadata(&path)?.len() == 0 {
            return Err(format!("Не указано или пустое фото: {}", path.display()).into());
        }
        photos.push(path);
    }

    Ok(Campaign { id: id.to_owned(), message, photos })
}

fn state_path(id: &str) -> PathBuf {
    Path::new("data").join("broadcast_state").join(format!("{id}.json"))
}

fn load_state(path: &Path) -> AppResult<BroadcastState> {
    if path.exists() {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    } else {
        Ok(BroadcastState::default())
    }
}

fn save_state(path: &Path, state: &BroadcastState) -> AppResult<()> {
    let parent = path.parent().ok_or("Некорректный путь")?;
    fs::create_dir_all(parent)?;
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    fs::write(&tmp, serde_json::to_vec_pretty(state)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn acquire_lock(id: &str) -> AppResult<LockGuard> {
    let dir = Path::new("data").join("broadcast_state");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{id}.lock"));
    OpenOptions::new().write(true).create_new(true).open(&path)
        .map_err(|e| format!("Cannot acquire broadcast lock {}: {e}", path.display()))?;
    Ok(LockGuard(path))
}

fn deterministic_random_id(id: &str, peer_id: i64) -> i64 {
    let mut hash = 2166136261_u32;
    for byte in format!("{id}:{peer_id}").bytes() {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16777619);
    }
    i64::from((hash & 0x7fff_ffff).max(1))
}

async fn send_vk_message(token: &str, peer_id: i64, message: &str, attachments: &[String], random_id: i64) -> AppResult<()> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(40)).build()?;
    let form = [
        ("access_token", token.to_owned()),
        ("v", VK_API_VERSION.to_owned()),
        ("peer_id", peer_id.to_string()),
        ("random_id", random_id.to_string()),
        ("message", message.to_owned()),
        ("attachment", attachments.join(",")),
    ];

    let response = client.post("https://api.vk.com/method/messages.send")
        .form(&form)
        .send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(format!("VK HTTP {}: {}", status, body).into());
    }

    let value: Value = serde_json::from_str(&body)?;
    if let Some(error) = value.get("error") {
        return Err(format!("VK API error: {}", error).into());
    }
    if value.get("response").is_none() {
        return Err(format!("VK returned no response: {}", value).into());
    }
    Ok(())
}

async fn send_to_peer(token: &str, campaign: &Campaign, peer_id: i64, random_id: i64) -> AppResult<()> {
    let mut attachments = Vec::new();
    for path in &campaign.photos {
        let attachment = photo::upload_photo(token, peer_id, path).await?;
        attachments.push(attachment);
        sleep(Duration::from_millis(650)).await;
    }
    send_vk_message(token, peer_id, &campaign.message, &attachments, random_id).await
}

fn load_recipients() -> AppResult<Vec<i64>> {
    let users: Vec<i64> = serde_json::from_slice(&fs::read("data/users.json")?)?;
    let unique: BTreeSet<_> = users.into_iter().filter(|&peer| peer > 0 && peer < 2_000_000_000).collect();
    Ok(unique.into_iter().collect())
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Использование: broadcast <campaign-id> --check | --test <peer-id> | --send");
        return Err("Invalid arguments".into());
    }

    let campaign = load_campaign(&args[1])?;
    println!("Campaign: {}", campaign.id);
    println!("Text length: {} chars", campaign.message.chars().count());
    println!("Photos: {}", campaign.photos.len());
    for path in &campaign.photos {
        println!("  {}", path.display());
    }

    if args[2] == "--check" && args.len() == 3 {
        println!("Validation successful. No messages were sent.");
        return Ok(());
    }

    dotenvy::dotenv().ok();
    let token = env::var("VK_TOKEN")?;

    if args[2] == "--test" && args.len() == 4 {
        let peer_id: i64 = args[3].parse()?;
        let random_id = i64::from((random::<u32>() & 0x7fff_ffff).max(1));
        println!("Sending TEST to peer_id={peer_id}");
        send_to_peer(&token, &campaign, peer_id, random_id).await?;
        println!("Test sent; production state unchanged.");
        return Ok(());
    }

    if args[2] != "--send" || args.len() != 3 {
        return Err("Invalid arguments".into());
    }

    let _lock = acquire_lock(&campaign.id)?;
    let path = state_path(&campaign.id);
    let mut state = load_state(&path)?;
    let recipients = if state.recipients.is_empty() { load_recipients()? } else { state.recipients.clone() };
    let pending: Vec<_> = recipients.iter().copied().filter(|peer| !state.sent.contains(peer)).collect();
    println!("Recipients in campaign audience: {}", recipients.len());
    println!("Already successful: {}", state.sent.len());
    println!("Pending: {}", pending.len());

    if pending.is_empty() {
        println!("Nothing to send.");
        return Ok(());
    }

    print!("Type SEND to broadcast to the pending recipients: ");
    io::stdout().flush()?;
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;
    if confirmation.trim() != "SEND" {
        println!("Cancelled. No messages were sent.");
        return Ok(());
    }

    if state.recipients.is_empty() {
        state.recipients = recipients;
        save_state(&path, &state)?;
    }

    let mut ok = 0_usize;
    let mut failed = 0_usize;
    for (index, peer_id) in pending.iter().enumerate() {
        println!("[{}/{}] peer_id={}", index + 1, pending.len(), peer_id);
        let random_id = deterministic_random_id(&campaign.id, *peer_id);
        match send_to_peer(&token, &campaign, *peer_id, random_id).await {
            Ok(()) => {
                state.sent.insert(*peer_id);
                state.failed.remove(peer_id);
                ok += 1;
                println!("  OK");
            }
            Err(error) => {
                state.failed.insert(*peer_id, error.to_string());
                failed += 1;
                eprintln!("  ERROR: {error}");
            }
        }
        save_state(&path, &state)?;
        sleep(Duration::from_millis(SEND_DELAY_MS)).await;
    }
    println!("Finished: successful this run={ok}, failed this run={failed}, total successful={}", state.sent.len());
    println!("State: {}", path.display());
    Ok(())
}
