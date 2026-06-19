use core::fmt;

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Form, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;

use super::{ApiResponse, AppState};
use crate::content::note::{Note, NoteSummary};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_notes))
        .route("/:slug", get(get_note))
        .route("/", post(create_note))
        .layer(CompressionLayer::new())
}

#[derive(Debug, Serialize)]
pub(super) struct NotesResponse {
    notes: Vec<NoteSummary>,
}

pub async fn list_notes(State(state): State<AppState>) -> ApiResponse<NotesResponse> {
    let notes: Vec<NoteSummary> = state.note_index.notes();

    ApiResponse::JsonData(NotesResponse { notes })
}

async fn get_note(State(state): State<AppState>, Path(slug): Path<String>) -> ApiResponse<Note> {
    let note_option = state.note_index.note(&slug);

    match note_option {
        Some(note) => ApiResponse::JsonData(note),
        None => ApiResponse::NotFound,
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MicropubType {
    #[serde(rename = "entry")]
    Entry,
}

impl fmt::Display for MicropubType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entry => write!(f, "entry"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MicropubFormRequest {
    pub action: String,
    pub h: MicropubType,
    pub content: String,
}

#[derive(Serialize)]
pub struct CreatedNoteResponse {
    content: String,
    r#type: Vec<String>,
}

pub async fn create_note(
    Form(form): Form<MicropubFormRequest>,
) -> ApiResponse<CreatedNoteResponse> {
    let note = CreatedNoteResponse {
        r#type: vec![format!("h-{}", form.h)],
        content: form.content,
    };

    ApiResponse::JsonData(note)
}
