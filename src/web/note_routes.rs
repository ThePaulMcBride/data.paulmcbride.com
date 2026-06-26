use core::fmt;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Form, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;

use super::{ApiResponse, AppState};
use crate::content::note::{Note, NoteGroup, NoteSummary};

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
    let notes: Vec<NoteSummary> = state.note_index.notes();

    ApiResponse::JsonData(NotesResponse { notes })
}

#[derive(Debug, Deserialize)]
pub(super) struct NotePageQuery {
    after: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(super) struct NotePageResponse {
    items: Vec<NoteGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_cursor: Option<String>,
}

pub async fn list_note_page(
    State(state): State<AppState>,
    Query(query): Query<NotePageQuery>,
) -> ApiResponse<NotePageResponse> {
    let groups = state.note_index.note_groups();
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let cursor = query.after.as_deref();
    let cursor_index = cursor.and_then(|cursor| {
        groups
            .iter()
            .position(|group| group_contains_slug(group, cursor))
    });
    let start_index = cursor_index.map(|index| index + 1).unwrap_or(0);
    let items = groups
        .iter()
        .skip(start_index)
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let next_cursor = if start_index + limit < groups.len() {
        items.last().and_then(group_cursor)
    } else {
        None
    };
    let previous_page_start = start_index.saturating_sub(limit);
    let previous_cursor = if start_index == 0 {
        None
    } else if previous_page_start == 0 {
        Some(String::new())
    } else {
        groups.get(previous_page_start - 1).and_then(group_cursor)
    };

    ApiResponse::JsonData(NotePageResponse {
        items,
        next_cursor,
        previous_cursor,
    })
}

fn group_contains_slug(group: &NoteGroup, slug: &str) -> bool {
    group.notes.iter().any(|note| note.slug == slug)
}

fn group_cursor(group: &NoteGroup) -> Option<String> {
    group.notes.last().map(|note| note.slug.clone())
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
