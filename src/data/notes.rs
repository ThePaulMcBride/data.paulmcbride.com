use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NoteSummary {
    pub slug: String,
    pub title: String,
}

#[derive(Serialize, Deserialize)]
pub struct Note {
    pub r#type: Vec<String>,
    pub properties: NoteProperties,
}

#[derive(Serialize, Deserialize)]
pub struct IdentifiableNote {
    pub slug: String,
    pub note: Note,
}

#[derive(Serialize, Deserialize)]
pub struct NoteProperties {
    pub content: Vec<String>,
}

pub struct NoteService;

impl NoteService {
    pub fn get_notes() -> Vec<NoteSummary> {
        vec![
            NoteSummary {
                slug: "first-note".to_string(),
                title: "First Note".to_string(),
            },
            NoteSummary {
                slug: "second-note".to_string(),
                title: "Second Note".to_string(),
            },
        ]
    }
}
