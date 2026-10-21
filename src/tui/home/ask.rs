//! The questions the workspace asks in a text popup, and the request each answer becomes.

use super::model::Act;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    AddProject,
    NewProfile,
    SaveProfile,
    AddEntry,
}

impl Ask {
    pub fn title(self) -> &'static str {
        match self {
            Ask::AddProject => "Add project",
            Ask::NewProfile => "New profile",
            Ask::SaveProfile => "Save setup as profile",
            Ask::AddEntry => "Add to the profile",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Ask::AddProject => "path",
            Ask::NewProfile | Ask::SaveProfile => "name",
            Ask::AddEntry => "skill SOURCE NAMES | plugin SOURCE MARKETPLACE NAME",
        }
    }

    /// The request for `answer`, given the project and profile the cursors are on.
    pub fn act(self, answer: String, project: usize, profile: usize) -> Act {
        match self {
            Ask::AddProject => Act::AddProject(answer),
            Ask::NewProfile => Act::NewProfile(answer),
            Ask::SaveProfile => Act::SaveProfile {
                name: answer,
                project,
            },
            Ask::AddEntry => Act::AddTyped {
                profile,
                spec: answer,
            },
        }
    }
}
