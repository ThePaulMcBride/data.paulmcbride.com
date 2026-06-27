use core::fmt;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Form, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;

use super::{ApiResponse, AppState};
use crate::content::note::{NoteGroup, NotePage, NoteSummary};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_notes))
        .route("/page", get(list_note_page))
        .route("/:slug", get(get_note))
        .route("/", post(create_note))
        .layer(CompressionLayer::new())
}

#[derive(Debug, Serialize)]
pub(super) struct NotesResponse {
    notes: Vec<NoteSummary>,
}

pub async fn list_notes(State(state): State<AppState>) -> ApiResponse<NotesResponse> {
    let notes: Vec<NoteSummary> = state.notes();

    ApiResponse::JsonData(NotesResponse { notes })
}

#[derive(Debug, Deserialize)]
pub(super) struct NotePageQuery {
    after: Option<String>,
    limit: Option<usize>,
}

pub async fn list_note_page(
    State(state): State<AppState>,
    Query(query): Query<NotePageQuery>,
) -> ApiResponse<NotePage> {
    let limit = query.limit.unwrap_or(25).clamp(1, 100);

    ApiResponse::JsonData(state.note_page(query.after.as_deref(), limit))
}

async fn get_note(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResponse<NoteGroup> {
    let group_option = state.note_group(&slug);

    match group_option {
        Some(group) => ApiResponse::JsonData(group),
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
