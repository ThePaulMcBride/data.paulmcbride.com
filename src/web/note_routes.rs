use core::fmt;

use axum::{
    routing::{get, post},
    Form, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;

use super::ApiResponse;
use crate::data::notes::{IdentifiableNote, Note, NoteProperties, NoteService, NoteSummary};

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_notes))
        .route("/", post(create_note))
        .layer(CompressionLayer::new())
}

#[derive(Debug, Serialize)]
pub struct NotesResponse {
    notes: Vec<NoteSummary>,
}

pub async fn list_notes() -> ApiResponse<NotesResponse> {
    let notes: Vec<NoteSummary> = NoteService::get_notes();

    ApiResponse::JsonData(NotesResponse { notes })
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

pub async fn create_note(Form(form): Form<MicropubFormRequest>) -> ApiResponse<Note> {
    let current_time_string = chrono::Utc::now().timestamp().to_string();

    let identifiable_note = IdentifiableNote {
        slug: current_time_string.clone(),
        note: Note {
            r#type: vec![format!("h-{}", form.h)],
            properties: NoteProperties {
                content: vec![form.content],
            },
        },
    };

    ApiResponse::JsonData(identifiable_note.note)
}
