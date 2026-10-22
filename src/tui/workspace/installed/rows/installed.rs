//! The items a scope has installed, each with the package it belongs to.

use super::Item;
use crate::library::ItemKind;
use crate::location::detail;
use crate::merge::tag;
use crate::model::{Augmentation, Kind};
use crate::package::Package;

pub(super) fn items(here: &[Augmentation]) -> Vec<(Package, Item)> {
    here.iter()
        .filter_map(|installed| {
            let kind = ItemKind::of(installed.kind)?;
            Some((Package::of(installed), item(installed, kind)))
        })
        .collect()
}

fn item(installed: &Augmentation, kind: ItemKind) -> Item {
    Item {
        label: installed.name.clone(),
        kind,
        agents: tag(&installed.agents),
        detail: match installed.kind {
            Kind::Plugin => provides(installed),
            _ => detail(installed),
        },
        id: installed.id.clone(),
    }
}

/// What a plugin brings along: `3 skills, 2 hooks`.
fn provides(plugin: &Augmentation) -> String {
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
