use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast;

use tesela_core::{db::SqliteIndex, storage::filesystem::FsNoteStore, types::TypeRegistry, Note};

pub struct AppState {
    pub store: Arc<FsNoteStore>,
    pub index: Arc<SqliteIndex>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub type_registry: TypeRegistry,
}

/// Events broadcast to WebSocket clients when notes change.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum WsEvent {
    NoteCreated { note: Note },
    NoteUpdated { note: Note },
    NoteDeleted { id: String },
}
