use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
    routing::{get, post},
    Router,
};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::AppHandle;

use crate::audio::import::{
    start_import, ImportResult, ImportError, ImportWarning,
};

// We use a concrete type alias to keep things simple on macOS/Windows/Linux.
pub type SharedHttpState = Arc<Mutex<Option<AppHandle<tauri::Wry>>>>;

/// Request body to import an audio file.
#[derive(Debug, Deserialize)]
pub struct ImportAudioRequest {
    /// Absolute path to the audio file to transcribe.
    pub source_path: String,
    /// Optional meeting title. Defaults to the file name.
    #[serde(default)]
    pub title: Option<String>,
    /// Optional language code, e.g. "en", "es". Null/omitted means auto.
    #[serde(default)]
    pub language: Option<String>,
    /// Optional model name. Null/omitted means use the configured default.
    #[serde(default)]
    pub model: Option<String>,
    /// Optional provider: "whisper" or "parakeet". Defaults to whisper.
    #[serde(default)]
    pub provider: Option<String>,
}

/// Response body for import status.
#[derive(Debug, Serialize)]
pub struct ImportAudioResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meeting_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Start importing (transcribing) an audio file.
async fn import_audio_handler(
    State(state): State<SharedHttpState>,
    Json(payload): Json<ImportAudioRequest>,
) -> (StatusCode, JsonResponse<ImportAudioResponse>) {
    info!("HTTP API: import audio request for {}", payload.source_path);

    let app_handle = {
        let guard = state.lock().await;
        match guard.as_ref() {
            Some(ah) => ah.clone(),
            None => {
                error!("HTTP API: app handle not available");
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    JsonResponse(ImportAudioResponse {
                        success: false,
                        message: "Meetily is not fully initialized yet".to_string(),
                        meeting_id: None,
                        error: Some("app handle not available".to_string()),
                    }),
                );
            }
        }
    };

    // Derive title if not provided.
    let title = payload.title.unwrap_or_else(|| {
        std::path::Path::new(&payload.source_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Audio")
            .to_string()
    });

    let source_path = payload.source_path.clone();
    let language = payload.language;
    let model = payload.model;
    let provider = payload.provider.unwrap_or_else(|| "whisper".to_string());

    // Spawn the import in the background and return immediately.
    let app_for_import = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        // Set up one-off event listeners so we can log completion/failure.
        let app_for_events = app_for_import.clone();
        tauri::async_runtime::spawn(async move {
            use tauri::Listener;
            let _ = app_for_events.listen("import-complete", |event| {
                if let Ok(payload) = serde_json::from_str::<ImportResult>(event.payload()) {
                    info!(
                        "HTTP API: import complete meeting_id={} segments={}",
                        payload.meeting_id, payload.segments_count
                    );
                }
            });
            let _ = app_for_events.listen("import-error", |event| {
                if let Ok(payload) = serde_json::from_str::<ImportError>(event.payload()) {
                    error!("HTTP API: import error {}", payload.error);
                }
            });
            let _ = app_for_events.listen("import-warning", |event| {
                if let Ok(payload) = serde_json::from_str::<ImportWarning>(event.payload()) {
                    warn!("HTTP API: import warning {} - {:?}", payload.warning, payload.details);
                }
            });
        });

        let result = start_import(
            app_for_import,
            source_path,
            title,
            language,
            model,
            Some(provider),
        )
        .await;

        if let Err(e) = result {
            error!("HTTP API: background import failed: {}", e);
        }
    });

    (
        StatusCode::ACCEPTED,
        JsonResponse(ImportAudioResponse {
            success: true,
            message: "Import started in the background".to_string(),
            meeting_id: None,
            error: None,
        }),
    )
}

/// Health check endpoint.
async fn health_handler() -> (StatusCode, JsonResponse<HealthResponse>) {
    (
        StatusCode::OK,
        JsonResponse(HealthResponse {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    )
}

/// Build the HTTP API router.
pub fn create_router(state: SharedHttpState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/import-audio", post(import_audio_handler))
        .with_state(state)
}

/// Start the HTTP server bound to localhost.
///
/// The server runs on `MEETILY_HTTP_PORT` (default 9517).
pub async fn start_http_server(state: SharedHttpState) -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::var("MEETILY_HTTP_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9517);

    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;
    let app = create_router(state);

    info!("Starting Meetily HTTP API on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
