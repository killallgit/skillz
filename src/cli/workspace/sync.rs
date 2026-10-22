//! Filling the library from what is installed while the workspace is already on screen.

use std::sync::mpsc::Sender;

use crate::cli::deps::Deps;
use crate::cli::session::Notice;
use crate::env::Env;
use crate::sync::{self, Report};
use crate::tui::workspace::Synced;

/// Scan and sync on a thread of their own and send the outcome to `tell`, unless the deps have
/// no scanner.
pub(super) fn start(deps: &Deps, env: &Env, tell: Sender<Synced>) {
    if let Some(scanner) = &deps.scanner {
        sync::spawn(scanner.clone(), env.clone(), move |result| {
            let _ = tell.send(synced_of(result));
        });
    }
}

/// The library to show and the toast to give: only news is toasted, and a failure quietly.
fn synced_of(result: anyhow::Result<Report>) -> Synced {
    match result {
        Ok(Report { synced, library }) => {
            let (added, secrets) = (synced.added.len(), synced.secrets_skipped());
            let news = added > 0 || secrets > 0;
            Synced {
                library: Some(library),
                notice: news.then(|| Notice::Synced { added, secrets }.to_string()),
            }
        }
        Err(e) => Synced {
            library: None,
            notice: Some(Notice::Failed(format!("library sync failed: {e:#}")).to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{Catalog, LibraryItem, Synced as Found};
    use crate::profile::Entry;

    fn report_adding(count: usize) -> Report {
        let item = LibraryItem::from_entry(Entry::Hook {
            event: "Stop".into(),
            matcher: None,
            command: "fmt".into(),
            agents: vec![],
        });
        Report {
            synced: Found {
                added: vec![item; count],
                skipped: vec![],
            },
            library: Catalog::default(),
        }
    }

    #[test]
    fn new_items_are_toasted_and_the_library_is_handed_over() {
        let synced = synced_of(Ok(report_adding(2)));

        assert!(synced.library.is_some() && synced.notice.is_some());
    }

    #[test]
    fn a_sync_that_found_nothing_new_is_silent() {
        let synced = synced_of(Ok(report_adding(0)));

        assert!(synced.library.is_some() && synced.notice.is_none());
    }

    #[test]
    fn a_failed_sync_toasts_and_leaves_the_library_alone() {
        let synced = synced_of(Err(anyhow::anyhow!("corrupt")));

        assert!(synced.library.is_none() && synced.notice.is_some());
    }
}
