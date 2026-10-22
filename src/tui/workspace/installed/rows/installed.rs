//! The items a scope has installed, each with the package it belongs to.

use super::{Item, Owned, Sources, State};
use crate::library::ItemKind;
use crate::location::detail;
use crate::merge::tag;
use crate::model::{Augmentation, Kind, split_plugin_id};
use crate::package::Package;
use crate::tui::workspace::installed::presence::has_counterpart;

pub(super) fn items(sources: &Sources) -> Vec<(Package, Item)> {
    sources
        .here
        .iter()
        .filter_map(|installed| {
            let kind = ItemKind::of(installed.kind)?;
            Some((Package::of(installed), item(installed, kind, sources)))
        })
        .collect()
}

fn item(installed: &Augmentation, kind: ItemKind, sources: &Sources) -> Item {
    Item {
        label: label(installed),
        kind,
        agents: tag(&installed.agents),
        detail: detail(installed),
        state: State::Installed(installed.id.clone()),
        global: has_counterpart(sources.also_global, installed),
        owned: owned(installed),
    }
}

/// A plugin is named by its marketplace's group already, so only its own name is shown.
fn label(installed: &Augmentation) -> String {
    match installed.kind {
        Kind::Plugin => split_plugin_id(&installed.name).0.to_string(),
        _ => installed.name.clone(),
    }
}

/// The skills then the hooks a plugin provides.
fn owned(plugin: &Augmentation) -> Vec<Owned> {
    if plugin.kind != Kind::Plugin {
        return vec![];
    }
    let provided = |label: String, kind| Owned {
        label,
        kind,
        detail: format!("added by {}", plugin.name),
    };
    let skills = plugin
        .plugin_skills()
        .map(|skill| provided(skill.to_string(), ItemKind::Skill));
    let hooks = plugin
        .hooks
        .iter()
        .map(|hook| provided(hook.label(), ItemKind::Hook));
    skills.chain(hooks).collect()
}
