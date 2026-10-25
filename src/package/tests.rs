use super::*;
use crate::harness::model::{Agent, Installer, Scope};

fn item(kind: Kind, name: &str, source: Option<&str>, group: &str) -> Installed {
    Installed {
        details: Default::default(),
        id: format!("{kind}:{name}"),
        name: name.into(),
        kind,
        agents: vec![Agent::Claude],
        scope: Scope::Project,
        group: group.into(),
        source: source.map(Into::into),
        installer: Installer::Npx,
        origin: Default::default(),
        children: vec![],
        hooks: vec![],
        members: vec![],
        removal: vec![],
    }
}

fn packages(items: Vec<Installed>) -> Vec<Package> {
    group_by(items, Package::of)
        .into_iter()
        .map(|(package, _)| package)
        .collect()
}

#[test]
fn skills_and_subagents_of_one_repo_share_a_package() {
    let skill = item(Kind::Skill, "tdd", Some("o/r"), "o/r");
    let subagent = item(Kind::Subagent, "reviewer", Some("o/r"), "o/r");

    let grouped = group_by(vec![skill, subagent], Package::of);

    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped[0].0, Package::Source("o/r".into()));
    assert_eq!(grouped[0].1.len(), 2);
}

#[test]
fn a_marketplace_and_its_plugins_share_a_package() {
    let marketplace = item(Kind::Marketplace, "mp", Some("o/mp"), "mp");
    let plugin = item(Kind::Plugin, "p@mp", Some("o/mp"), "mp");

    let found = packages(vec![marketplace, plugin]);

    assert_eq!(found, [Package::Marketplace("mp".into())]);
}

#[test]
fn items_with_no_source_are_local_files() {
    let by_hand = item(Kind::Skill, "mine", None, "local");

    assert_eq!(Package::of(&by_hand), Package::Local);
}

#[test]
fn mcp_servers_and_unowned_hooks_keep_a_group_each() {
    let mcp = item(Kind::Mcp, "fs", None, "mcp");
    let hook = item(Kind::Hook, "rtk", None, "hooks");

    let found = packages(vec![hook, mcp]);

    assert_eq!(found, [Package::Mcp, Package::Hooks]);
}

#[test]
fn packages_come_named_first_alphabetically_then_local_mcp_hooks() {
    let items = vec![
        item(Kind::Hook, "h", None, "hooks"),
        item(Kind::Skill, "a", None, "local"),
        item(Kind::Skill, "b", Some("zed/skills"), "zed/skills"),
        item(Kind::Mcp, "m", None, "mcp"),
        item(Kind::Skill, "c", Some("Acme/skills"), "Acme/skills"),
    ];

    let found = packages(items);

    assert_eq!(
        found,
        [
            Package::Source("Acme/skills".into()),
            Package::Source("zed/skills".into()),
            Package::Local,
            Package::Mcp,
            Package::Hooks
        ]
    );
}

#[test]
fn the_same_name_from_different_sources_is_two_packages() {
    let one = item(Kind::Skill, "tdd", Some("a/r"), "a/r");
    let other = item(Kind::Skill, "tdd", Some("b/r"), "b/r");

    assert_eq!(packages(vec![one, other]).len(), 2);
}

#[test]
fn members_keep_their_order() {
    let items = vec![
        item(Kind::Skill, "z", Some("o/r"), "o/r"),
        item(Kind::Skill, "a", Some("o/r"), "o/r"),
    ];

    let grouped = group_by(items, Package::of);

    let names: Vec<&str> = grouped[0].1.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["z", "a"]);
}

#[test]
fn a_library_skill_from_a_folder_on_disk_is_local_files() {
    let from_folder = Spec::Skill {
        source: "/work/skills".into(),
        names: vec!["mine".into()],
        agents: vec![],
    };

    assert_eq!(Package::of_entry(&from_folder), Package::Local);
}

#[test]
fn a_library_entry_belongs_to_the_package_it_is_fetched_from() {
    let skill = Spec::Skill {
        source: "o/r".into(),
        names: vec!["tdd".into()],
        agents: vec![],
    };
    let plugin = Spec::Plugin {
        source: "o/mp".into(),
        marketplace: "mp".into(),
        name: "p".into(),
        agents: vec![],
    };

    assert_eq!(Package::of_entry(&skill), Package::Source("o/r".into()));
    assert_eq!(Package::of_entry(&plugin), Package::Source("o/mp".into()));
}
