use serde::{Deserialize, Serialize};

fn news_api_url() -> String {
    format!("{}/launcher/news", crate::config::API_BASE)
}

#[derive(Deserialize)]
struct NewsApiResponse {
    success: bool,
    // Raw values, not Vec<NewsApiItem> — one malformed item must not fail the whole page,
    // especially get_more_news() below, which has no cache fallback to fall back on
    items: Vec<serde_json::Value>,
    #[serde(rename = "hasMore", default)]
    has_more: bool,
    #[serde(rename = "lastId")]
    last_id: Option<String>,
}

#[derive(Deserialize)]
struct NewsApiItem {
    id: String,
    title: String,
    content: Option<String>,
    preview: Option<String>,
    #[serde(rename = "imageUrl")]
    image_url: Option<String>,
    #[serde(rename = "authorName")]
    author_name: String,
    #[serde(rename = "authorRole")]
    author_role: Option<String>,
    #[serde(rename = "authorAvatarUrl")]
    author_avatar_url: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: String,
    category: String,
    url: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NewsItem {
    id: String,
    title: String,
    content: String,
    excerpt: String,
    #[serde(rename = "imageUrl")]
    image_url: Option<String>,
    author: String,
    #[serde(rename = "authorRole")]
    author_role: String,
    #[serde(rename = "authorAvatarUrl")]
    author_avatar_url: Option<String>,
    date: String,
    tag: String,
    url: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NewsPage {
    items: Vec<NewsItem>,
    #[serde(rename = "hasMore")]
    has_more: bool,
    #[serde(rename = "lastId")]
    last_id: Option<String>,
}

fn generate_excerpt(content: &str, max_len: usize) -> String {
    let stripped = content.replace("**", "").replace("__", "").replace("~~", "").replace('`', "");
    let unheaded = stripped
        .lines()
        .map(|line| line.trim_start().trim_start_matches('#').trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    let trimmed = unheaded.trim();
    let char_count = trimmed.chars().count();
    if char_count <= max_len {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_len).collect();
        format!("{}...", truncated.trim_end())
    }
}

fn category_label(category: &str) -> &'static str {
    match category {
        "update" => "Обновление",
        "event" => "Ивент",
        "announcement" => "Объявление",
        "maintenance" => "Сервер",
        _ => "Комьюнити",
    }
}

const NEWS_FALLBACK_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);

fn news_cache_path() -> std::path::PathBuf {
    crate::game::game_dir().join("news_cache.json")
}

#[tauri::command]
pub fn get_cached_news() -> Option<NewsPage> {
    std::fs::read_to_string(news_cache_path()).ok().and_then(|json| serde_json::from_str(&json).ok())
}

#[tauri::command]
pub async fn get_news() -> Result<NewsPage, String> {
    match get_news_live(None).await {
        Ok(page) => {
            let cached = page.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                if let Ok(json) = serde_json::to_string(&cached) {
                    let _ = std::fs::write(news_cache_path(), json);
                }
            })
            .await;
            Ok(page)
        }
        Err(err) => tauri::async_runtime::spawn_blocking(|| {
            crate::skin::read_cache_if_fresh(&news_cache_path(), NEWS_FALLBACK_MAX_AGE)
                .and_then(|json| serde_json::from_str(&json).ok())
        })
        .await
        .ok()
        .flatten()
        .ok_or(err),
    }
}

// Paginated requests bypass the disk cache entirely — only the first (cursor-less) page is cached
#[tauri::command]
pub async fn get_more_news(before: String) -> Result<NewsPage, String> {
    get_news_live(Some(before)).await
}

async fn get_news_live(before: Option<String>) -> Result<NewsPage, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|err| err.to_string())?;
    let mut req = client.get(news_api_url()).query(&[("limit", "8")]);
    if let Some(before) = &before {
        req = req.query(&[("before", before.as_str())]);
    }
    let res = req.send().await.map_err(|err| err.to_string())?;

    if !res.status().is_success() {
        return Err(format!("News API returned {}", res.status()));
    }

    let parsed: NewsApiResponse = res.json().await.map_err(|err| err.to_string())?;
    if !parsed.success {
        return Err("News API returned success=false".to_string());
    }

    let items = parsed
        .items
        .into_iter()
        .filter_map(|raw| match serde_json::from_value::<NewsApiItem>(raw) {
            Ok(item) => Some(item),
            Err(err) => {
                log::warn!("skipping malformed news item: {err}");
                None
            }
        })
        .map(|item| {
            let content = item.content.unwrap_or_default();
            let excerpt = if content.is_empty() {
                item.preview.unwrap_or_default().chars().take(160).collect()
            } else {
                generate_excerpt(&content, 160)
            };
            NewsItem {
                id: item.id,
                title: item.title,
                content,
                excerpt,
                image_url: item.image_url,
                author: item.author_name,
                author_role: item.author_role.unwrap_or_default(),
                author_avatar_url: item.author_avatar_url,
                date: item.published_at,
                tag: category_label(&item.category).to_string(),
                url: item.url,
            }
        })
        .collect();

    Ok(NewsPage { items, has_more: parsed.has_more, last_id: parsed.last_id })
}
