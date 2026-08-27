use axum::{
    extract::{Json, Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use teloxide::prelude::*;
use teloxide::types::InputFile;

#[derive(Clone)]
struct AppState {
    bot: Bot,
    client: Client,
    forward_url: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    
    let token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();
    if token.is_empty() || token == "your_telegram_bot_token_here" {
        println!("TELEGRAM_BOT_TOKEN not configured. Exiting.");
        return;
    }

    let forward_url = std::env::var("WEBHOOK_FORWARD_URL")
        .unwrap_or_else(|_| "http://backend-rust:25333/api/v1/webhook/telegram".to_string());

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(1800))
        .build()
        .unwrap_or_else(|_| Client::new());

    let teloxide_client = teloxide::net::default_reqwest_settings()
        .timeout(std::time::Duration::from_secs(1800))
        .build()
        .unwrap();

    let mut bot = Bot::with_client(token, teloxide_client);
    if let Ok(api_url_str) = std::env::var("TELEGRAM_API_URL") {
        let trimmed = api_url_str.trim();
        if !trimmed.is_empty() {
            match trimmed.parse::<reqwest::Url>() {
                Ok(api_url) => {
                    println!("Using custom Telegram Bot API Server: {}", api_url);
                    bot = bot.set_api_url(api_url);
                }
                Err(e) => {
                    eprintln!("Invalid TELEGRAM_API_URL '{}': {}. Falling back to default cloud API.", trimmed, e);
                }
            }
        }
    }

    let state = AppState {
        bot,
        client,
        forward_url,
    };

    let app = Router::new()
        .route("/", get(|| async { "Teras API Wrapper is running" }))
        .route("/health", get(|| async { "OK" }))
        .route("/api/webhook/setup", get(setup_webhook))
        .route("/api/webhook", post(handle_webhook))
        .route("/api/message/send", post(send_message))
        .route("/api/message/delete", post(delete_message))
        .route("/api/message/edit-caption", post(edit_caption))
        .route("/api/action/typing", post(send_typing_action))
        .route("/api/media/send", post(send_media))
        .route("/api/media/download", get(download_media))
        .layer(axum::extract::DefaultBodyLimit::max(2048 * 1024 * 1024))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    println!("Teras wrapper listening on 0.0.0.0:3001");
    axum::serve(listener, app).await.unwrap();
}

async fn setup_webhook(State(state): State<AppState>) -> impl IntoResponse {
    let domain = std::env::var("DOMAIN").unwrap_or_else(|_| "localhost".to_string());
    let url = format!("https://{}/api/webhook", domain);
    
    match state.bot.set_webhook(url.parse().unwrap()).await {
        Ok(_) => (StatusCode::OK, format!("Webhook successfully set to {}", url)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set webhook: {}", e)),
    }
}

// Forward the exact raw JSON from Telegram to backend-rust
async fn handle_webhook(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    println!("Received webhook from Telegram, forwarding to {}", state.forward_url);
    // Fire and forget so we can respond 200 OK immediately to Telegram
    let client = state.client.clone();
    let url = state.forward_url.clone();
    let payload = body.to_vec();

    tokio::spawn(async move {
        match client.post(&url)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await 
        {
            Ok(resp) => {
                println!("Successfully forwarded webhook. Status: {}", resp.status());
                if !resp.status().is_success() {
                    eprintln!("Failed to forward webhook to {}. Status: {}", url, resp.status());
                }
            }
            Err(e) => eprintln!("Error forwarding webhook to {}: {}", url, e),
        }
    });

    StatusCode::OK
}

#[derive(Deserialize)]
struct SendMessageReq {
    chat_id: i64,
    text: String,
}

#[derive(Serialize)]
struct ApiResponse {
    status: String,
    error: Option<String>,
    message_id: Option<i32>,
}

async fn send_message(
    State(state): State<AppState>,
    Json(payload): Json<SendMessageReq>,
) -> Json<ApiResponse> {
    match state.bot.send_message(ChatId(payload.chat_id), &payload.text).await {
        Ok(msg) => Json(ApiResponse { status: "success".to_string(), error: None, message_id: Some(msg.id.0) }),
        Err(e) => {
            eprintln!("Failed to send message: {}", e);
            Json(ApiResponse { status: "error".to_string(), error: Some(e.to_string()), message_id: None })
        }
    }
}

#[derive(Deserialize)]
struct SendTypingReq {
    chat_id: i64,
}

async fn send_typing_action(
    State(state): State<AppState>,
    Json(payload): Json<SendTypingReq>,
) -> Json<ApiResponse> {
    match state.bot.send_chat_action(ChatId(payload.chat_id), teloxide::types::ChatAction::Typing).await {
        Ok(_) => Json(ApiResponse { status: "success".to_string(), error: None, message_id: None }),
        Err(e) => {
            eprintln!("Failed to send typing action: {}", e);
            Json(ApiResponse { status: "error".to_string(), error: Some(e.to_string()), message_id: None })
        }
    }
}

#[derive(Deserialize)]
struct DeleteMessageReq {
    chat_id: i64,
    message_id: i32,
}

async fn delete_message(
    State(state): State<AppState>,
    Json(payload): Json<DeleteMessageReq>,
) -> Json<ApiResponse> {
    match state.bot.delete_message(ChatId(payload.chat_id), teloxide::types::MessageId(payload.message_id)).await {
        Ok(_) => Json(ApiResponse { status: "success".to_string(), error: None, message_id: Some(payload.message_id) }),
        Err(e) => {
            eprintln!("Failed to delete message: {}", e);
            Json(ApiResponse { status: "error".to_string(), error: Some(e.to_string()), message_id: None })
        }
    }
}

#[derive(Deserialize)]
struct EditCaptionReq {
    chat_id: i64,
    message_id: i32,
    caption: String,
}

async fn edit_caption(
    State(state): State<AppState>,
    Json(payload): Json<EditCaptionReq>,
) -> Json<ApiResponse> {
    match state.bot.edit_message_caption(ChatId(payload.chat_id), teloxide::types::MessageId(payload.message_id))
        .caption(payload.caption)
        .await
    {
        Ok(msg) => Json(ApiResponse { status: "success".to_string(), error: None, message_id: Some(msg.id.0) }),
        Err(e) => {
            eprintln!("Failed to edit caption: {}", e);
            Json(ApiResponse { status: "error".to_string(), error: Some(e.to_string()), message_id: None })
        }
    }
}

async fn send_media(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Json<ApiResponse> {
    let mut chat_id = 0;
    let mut media_type = String::from("document");
    let mut caption = None;
    let mut file_bytes = Vec::new();
    let mut filename = String::from("file");

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        if name == "chat_id" {
            if let Ok(text) = field.text().await {
                chat_id = text.parse().unwrap_or(0);
            }
        } else if name == "media_type" {
            if let Ok(text) = field.text().await {
                media_type = text;
            }
        } else if name == "caption" {
            if let Ok(text) = field.text().await {
                caption = Some(text);
            }
        } else if name == "file" || name == "data" {
            if let Some(fn_name) = field.file_name() {
                filename = fn_name.to_string();
            }
            file_bytes = field.bytes().await.unwrap_or_default().to_vec();
        }
    }

    if chat_id == 0 || file_bytes.is_empty() {
        return Json(ApiResponse { status: "error".to_string(), error: Some("Missing chat_id or file".to_string()), message_id: None });
    }

    let input_file = InputFile::memory(file_bytes).file_name(filename);
    let chat = ChatId(chat_id);

    let result = match media_type.as_str() {
        "photo" | "image" => {
            let mut req = state.bot.send_photo(chat, input_file);
            if let Some(cap) = caption { req = req.caption(cap); }
            req.await.map(|msg| msg.id.0)
        }
        "video" => {
            let mut req = state.bot.send_video(chat, input_file).supports_streaming(true);
            if let Some(cap) = caption { req = req.caption(cap); }
            req.await.map(|msg| msg.id.0)
        }
        _ => {
            let mut req = state.bot.send_document(chat, input_file);
            if let Some(cap) = caption { req = req.caption(cap); }
            req.await.map(|msg| msg.id.0)
        }
    };

    match result {
        Ok(msg_id) => Json(ApiResponse { status: "success".to_string(), error: None, message_id: Some(msg_id) }),
        Err(e) => {
            eprintln!("Failed to send media: {}", e);
            Json(ApiResponse { status: "error".to_string(), error: Some(e.to_string()), message_id: None })
        }
    }
}


#[derive(Deserialize)]
struct DownloadMediaQuery {
    file_id: String,
}

async fn download_media(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<DownloadMediaQuery>,
) -> impl IntoResponse {
    println!("Received download request for file_id: {}", query.file_id);
    match state.bot.get_file(query.file_id).await {
        Ok(file) => {
            println!("Got file path from Bot API: {}, size: {}", file.path, file.size);
            let base_api_url = state.bot.api_url();
            let base_str = base_api_url.as_str().trim_end_matches('/');
            let token_str = state.bot.token();

            let clean_path = file.path.trim_start_matches('/');
            let relative_file_path = if let Some(stripped) = clean_path.strip_prefix("var/lib/telegram-bot-api/") {
                stripped.strip_prefix(token_str).unwrap_or(stripped).trim_start_matches('/')
            } else if let Some(stripped) = clean_path.strip_prefix(token_str) {
                stripped.trim_start_matches('/')
            } else {
                clean_path
            };

            let url = format!("{}/file/bot{}/{}", base_str, token_str, relative_file_path);
            println!("Downloading stream from: {}", url);
            match state.client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        use futures_util::StreamExt;
                        let stream = resp.bytes_stream().map(|res| res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
                        let body = axum::body::Body::from_stream(stream);
                        let mut headers = axum::http::HeaderMap::new();
                        headers.insert(axum::http::header::CONTENT_TYPE, "application/octet-stream".parse().unwrap());
                        (StatusCode::OK, headers, body).into_response()
                    } else {
                        eprintln!("Failed to download from Bot API server. Status: {}", status);
                        (StatusCode::BAD_GATEWAY, format!("Failed to download from Bot API: {}", status)).into_response()
                    }
                }
                Err(e) => {
                    eprintln!("Reqwest error downloading from Bot API: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Reqwest error: {}", e)).into_response()
                }
            }
        }
        Err(e) => {
            eprintln!("Teloxide get_file error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Teloxide error: {}", e)).into_response()
        }
    }
}
