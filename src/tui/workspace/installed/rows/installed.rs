//! The items a scope has installed, each with the package it belongs to.

use super::Item;
use crate::app::library::ItemKind;
use crate::app::location::detail;
use crate::app::merge::tag;
use crate::app::package::Package;
use skillz_harness::model::{Installed, Kind};

pub(super) fn items(here: &[Installed]) -> Vec<(Package, Item)> {
    here.iter()
        .map(|installed| (Package::of(installed), item(installed, installed.kind)))
        .collect()
}

fn item(installed: &Installed, kind: Kind) -> Item {
    Item {
        label: installed.name.clone(),
        kind,
        agents: tag(&installed.agents),
        detail: match installed.kind {
            Kind::Plugin => provides(installed),
            _ => detail(installed),
        },
        id: installed.id.clone(),
        children: match kind {
            Kind::Plugin => installed
                .children
                .iter()
                .map(|child| skillz_harness::redact::redact_text(child))
                .collect(),
            _ => vec![],
        },
    }
}

/// What a plugin brings along: `3 skills, 2 hooks`.
fn provides(plugin: &Installed) -> String {
    let counts = [
        (plugin.plugin_skills().count(), ItemKind::Skill),
        (plugin.hooks.len(), ItemKind::Hook),
    ];
    let shown: Vec<String> = counts
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, kind)| match count {
            1 => format!("1 {}", kind.noun()),
            n => format!("{n} {}", kind.plural()),
        })
        .collect();
    shown.join(", ")
}
