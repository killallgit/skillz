//! The questions the workspace asks in a text popup, and the request each answer becomes.

use std::path::PathBuf;

use super::model::Act;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    NewProfile,
    SaveProfile,
    AddEntry,
}

impl Ask {
    pub fn title(self) -> &'static str {
        match self {
            Ask::NewProfile => "New profile",
            Ask::SaveProfile => "Save setup as profile",
            Ask::AddEntry => "Add to the profile",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Ask::NewProfile | Ask::SaveProfile => "name",
            Ask::AddEntry => "skill SOURCE NAMES | plugin SOURCE MARKETPLACE NAME",
        }
    }

    /// The request for `answer`, given the project and profile the cursors are on; `None`
    /// when it needs one of them and the cursor is on neither.
    pub fn act(
        self,
        answer: String,
        project: Option<PathBuf>,
        profile: Option<String>,
    ) -> Option<Act> {
        Some(match self {
            Ask::NewProfile => Act::NewProfile(answer),
            Ask::SaveProfile => Act::SaveProfile {
                name: answer,
                project: project?,
            },
            Ask::AddEntry => Act::AddTyped {
                profile: profile?,
                spec: answer,
            },
        })
    }
}
