use std::fmt;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::surge_client::{AddOutcome, ConnectError, ControlAction, Download, SurgeClient};

/// Largest integer exactly representable as a JavaScript `Number`. Byte counts
/// and timestamps from Surge are signed `int64`; values outside this range are
/// clamped so they survive the JSON/JS boundary without precision loss.
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Sanitized queue row sent to the frontend.
///
/// Only the download id, display filename, normalized status, and numeric
/// byte/speed metrics leave the process. The Surge `url` (may carry signed
/// secrets), `dest_path` (private filesystem layout), and `error` (server
/// text) are never projected. Field names cross the IPC boundary as camelCase.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItemDto {
    pub id: String,
    pub name: String,
    pub status: String,
    pub bytes_done: i64,
    pub bytes_total: i64,
    pub speed_bps: f64,
    pub added_at: i64,
}

impl fmt::Debug for QueueItemDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // id and name can be user-controlled and status is server-controlled,
        // so only the numeric metrics are safe to print.
        f.debug_struct("QueueItemDto")
            .field("bytes_done", &self.bytes_done)
            .field("bytes_total", &self.bytes_total)
            .field("speed_bps", &self.speed_bps)
            .field("added_at", &self.added_at)
            .finish()
    }
}

/// Bridge failure surfaced to the frontend as a stable, sanitized code.
///
/// No variant carries the endpoint, bearer token, response body, filename, or
/// error text, so `Serialize` and `Debug` are both safe to log or return.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BridgeError {
    InvalidEndpoint,
    EmptyToken,
    Unauthorized,
    Network,
    Timeout,
    BodyTooLarge,
    InvalidResponse,
    NotConnected,
    Superseded,
    State,
}

impl fmt::Debug for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::InvalidEndpoint => "InvalidEndpoint",
            Self::EmptyToken => "EmptyToken",
            Self::Unauthorized => "Unauthorized",
            Self::Network => "Network",
            Self::Timeout => "Timeout",
            Self::BodyTooLarge => "BodyTooLarge",
            Self::InvalidResponse => "InvalidResponse",
            Self::NotConnected => "NotConnected",
            Self::Superseded => "Superseded",
            Self::State => "State",
        };
        f.write_str(code)
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidEndpoint => "invalid Surge endpoint",
            Self::EmptyToken => "missing bearer token",
            Self::Unauthorized => "unauthorized: invalid bearer token",
            Self::Network => "connection failed",
            Self::Timeout => "connection timed out",
            Self::BodyTooLarge => "response body too large",
            Self::InvalidResponse => "invalid response from server",
            Self::NotConnected => "not connected to a Surge instance",
            Self::Superseded => "connection attempt was superseded",
            Self::State => "connection state unavailable",
        };
        f.write_str(message)
    }
}

impl std::error::Error for BridgeError {}

impl From<ConnectError> for BridgeError {
    fn from(err: ConnectError) -> Self {
        // Map known codes only; the `InvalidEndpoint` reason string is dropped
        // so no raw input can ride along with the error.
        match err {
            ConnectError::InvalidEndpoint(_) => Self::InvalidEndpoint,
            ConnectError::EmptyToken => Self::EmptyToken,
            ConnectError::Unauthorized => Self::Unauthorized,
            ConnectError::Network => Self::Network,
            ConnectError::Timeout => Self::Timeout,
            ConnectError::BodyTooLarge => Self::BodyTooLarge,
            ConnectError::InvalidResponse => Self::InvalidResponse,
        }
    }
}

#[derive(Default)]
struct ConnectionState {
    epoch: u64,
    client: Option<Arc<SurgeClient>>,
}

/// Owns the single active Surge connection.
///
/// `epoch` is a monotonic fence: every `connect`/`disconnect` bumps it, and a
/// completed network call is discarded unless its epoch is still current. That
/// stops a slow earlier `connect` from overwriting a newer connection (or a
/// `disconnect`). The mutex is never held across an `await`.
#[derive(Default)]
pub struct ConnectionManager {
    state: Mutex<ConnectionState>,
}

impl fmt::Debug for ConnectionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let connected = match self.state.lock() {
            Ok(state) => state.client.is_some(),
            Err(poisoned) => poisoned.into_inner().client.is_some(),
        };
        f.debug_struct("ConnectionManager")
            .field("connected", &connected)
            .finish()
    }
}

impl ConnectionManager {
    /// Authenticate against the Surge instance and store the connection.
    ///
    /// The previous connection (if any) is cleared under the lock before the
    /// network round-trip, so a new attempt always fences off the old one.
    pub async fn connect(
        &self,
        endpoint: &str,
        token: &str,
    ) -> Result<Vec<QueueItemDto>, BridgeError> {
        let epoch = {
            let mut state = self.state.lock().map_err(|_| BridgeError::State)?;
            state.epoch = state.epoch.wrapping_add(1);
            state.client = None;
            state.epoch
        };

        // Network runs without the lock held.
        let client = SurgeClient::connect(endpoint, token).await?;
        let items = project(client.list_downloads());

        {
            let mut state = self.state.lock().map_err(|_| BridgeError::State)?;
            if state.epoch != epoch {
                return Err(BridgeError::Superseded);
            }
            state.client = Some(Arc::new(client));
        }

        Ok(items)
    }

    /// Re-query `/list` on the active connection.
    pub async fn refresh(&self) -> Result<Vec<QueueItemDto>, BridgeError> {
        let (client, epoch) = {
            let state = self.state.lock().map_err(|_| BridgeError::State)?;
            let client = state.client.clone().ok_or(BridgeError::NotConnected)?;
            (client, state.epoch)
        };

        // Network runs without the lock held.
        let downloads = client.refresh().await?;
        let items = project(&downloads);

        {
            let state = self.state.lock().map_err(|_| BridgeError::State)?;
            if state.epoch != epoch {
                return Err(BridgeError::Superseded);
            }
        }

        Ok(items)
    }

    /// Fresh queue listing; each call performs an authenticated `GET /list`.
    ///
    /// Delegates to [`Self::refresh`] so the existing epoch fence applies and
    /// no lock is held across the await.
    pub async fn list_downloads(&self) -> Result<Vec<QueueItemDto>, BridgeError> {
        self.refresh().await
    }

    /// Apply a released control action to the active connection.
    ///
    /// Mirrors [`Self::refresh`]: the retained client and the epoch are cloned
    /// under the lock, the network round trip runs without it, and the epoch
    /// is re-checked after a successful POST so a superseding
    /// `connect`/`disconnect` cannot let a stale acknowledgment surface as
    /// success. The cached queue snapshot is never mutated here; this only
    /// forwards the action and reports that Surge acknowledged it.
    pub async fn control_download(
        &self,
        action: ControlAction,
        id: &str,
    ) -> Result<(), BridgeError> {
        let (client, epoch) = {
            let state = self.state.lock().map_err(|_| BridgeError::State)?;
            let client = state.client.clone().ok_or(BridgeError::NotConnected)?;
            (client, state.epoch)
        };

        // Network runs without the lock held.
        client.control_download(action, id).await?;

        {
            let state = self.state.lock().map_err(|_| BridgeError::State)?;
            if state.epoch != epoch {
                return Err(BridgeError::Superseded);
            }
        }

        Ok(())
    }

    /// Enqueue a download on the active connection.
    ///
    /// Mirrors [`Self::control_download`]: the retained client and the epoch
    /// are cloned under the lock, the network round trip runs without it, and
    /// the epoch is re-checked after a successful POST so a superseding
    /// `connect`/`disconnect` cannot let a stale acknowledgment surface as
    /// success. The cached queue snapshot is never mutated here; this only
    /// forwards the request and reports Surge's sanitized outcome.
    pub async fn add_download(&self, url: &str) -> Result<AddOutcome, BridgeError> {
        let (client, epoch) = {
            let state = self.state.lock().map_err(|_| BridgeError::State)?;
            let client = state.client.clone().ok_or(BridgeError::NotConnected)?;
            (client, state.epoch)
        };

        // Network runs without the lock held.
        let outcome = client.add_download(url).await?;

        {
            let state = self.state.lock().map_err(|_| BridgeError::State)?;
            if state.epoch != epoch {
                return Err(BridgeError::Superseded);
            }
        }

        Ok(outcome)
    }

    /// Drop the active connection and fence off any in-flight `connect`.
    pub fn disconnect(&self) {
        // Recover from a poisoned lock: the only mutation is a full reset, so a
        // half-written prior state is safely overwritten.
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.epoch = state.epoch.wrapping_add(1);
        state.client = None;
    }
}

fn project(downloads: &[Download]) -> Vec<QueueItemDto> {
    downloads.iter().map(project_item).collect()
}

fn project_item(download: &Download) -> QueueItemDto {
    let bytes_total = clamp_index(download.total_size);
    let mut bytes_done = clamp_index(download.downloaded);
    if bytes_total > 0 && bytes_done > bytes_total {
        bytes_done = bytes_total;
    }
    QueueItemDto {
        id: download.id.clone(),
        name: download.filename.clone(),
        status: map_status(&download.status).to_string(),
        bytes_done,
        bytes_total,
        speed_bps: sanitize_speed(download.speed),
        added_at: clamp_index(download.added_at),
    }
}

/// Squeeze a signed `int64` metric into `[0, JS_MAX_SAFE_INTEGER]`.
fn clamp_index(value: i64) -> i64 {
    value.clamp(0, JS_MAX_SAFE_INTEGER)
}

/// Non-finite rates collapse to zero; finite rates are clamped into
/// `[0, JS_MAX_SAFE_INTEGER]`.
fn sanitize_speed(speed: f64) -> f64 {
    if speed.is_finite() {
        speed.clamp(0.0, JS_MAX_SAFE_INTEGER as f64)
    } else {
        0.0
    }
}

/// Explicit Surge status projection; anything unrecognized becomes `unknown`.
fn map_status(status: &str) -> &'static str {
    match status {
        "downloading" => "active",
        "queued" => "queued",
        "paused" => "paused",
        "completed" => "completed",
        "error" => "failed",
        "pausing" => "pausing",
        _ => "unknown",
    }
}

#[tauri::command]
pub async fn connect_surge(
    endpoint: String,
    token: String,
    state: tauri::State<'_, ConnectionManager>,
) -> Result<Vec<QueueItemDto>, BridgeError> {
    state.connect(&endpoint, &token).await
}

#[tauri::command]
pub async fn list_downloads(
    state: tauri::State<'_, ConnectionManager>,
) -> Result<Vec<QueueItemDto>, BridgeError> {
    state.list_downloads().await
}

#[tauri::command]
pub fn disconnect_surge(state: tauri::State<'_, ConnectionManager>) {
    state.disconnect();
}

/// Forward a queue control action over the active connection.
///
/// `action` is matched against a strict, case-sensitive allowlist; anything
/// unrecognized becomes a sanitized [`BridgeError::InvalidResponse`]. There is
/// no mapping to Surge's `/purge` route, so this command never explicitly
/// purges completed final files (released `/delete` cancels active transfers
/// and may clean partial bytes). Returns only an acknowledgment — the
/// frontend owns any queue refresh.
#[tauri::command]
pub async fn control_download(
    action: String,
    id: String,
    state: tauri::State<'_, ConnectionManager>,
) -> Result<(), BridgeError> {
    let action = match action.as_str() {
        "pause" => ControlAction::Pause,
        "resume" => ControlAction::Resume,
        "removeRecord" => ControlAction::RemoveRecord,
        _ => return Err(BridgeError::InvalidResponse),
    };
    state.control_download(action, &id).await
}

/// Enqueue a download on the active connection.
///
/// The URL is validated locally by the client before any network request; only
/// the sanitized [`AddOutcome`] crosses back to the frontend, and no response
/// field (`id`, `filename`, `status`, `message`) is projected. A stale
/// connection epoch surfaces as [`BridgeError::Superseded`] rather than a
/// stale success.
#[tauri::command]
pub async fn add_download(
    url: String,
    state: tauri::State<'_, ConnectionManager>,
) -> Result<AddOutcome, BridgeError> {
    state.add_download(&url).await
}
