use std::collections::HashSet;
use std::io::Write;

use super::deps::Deps;
use super::removal::{Review, confirm_and_remove};
use super::scope::Scene;
use crate::harness::cost;
use crate::harness::env::Env;
use crate::harness::mcp;
use crate::harness::model::{Installed, Kind};
use crate::harness::usage;
use crate::location::location;
use crate::package::Package;
use crate::prompt::Choice;
use crate::prune;
use crate::tree;
use crate::{info, remove};

pub(super) struct ListView<'a> {
    pub(super) query: Option<&'a str>,
    pub(super) all: bool,
    pub(super) json: bool,
    pub(super) check: bool,
    pub(super) interactive: bool,
}

/// Items `list QUERY` shows: the matches, plus the whole group of any matched marketplace.
fn queried(items: &[Installed], hits: &[&Installed]) -> Vec<Installed> {
    let ids: HashSet<String> = hits.iter().map(|i| i.id.clone()).collect();
    let groups: HashSet<String> = hits
        .iter()
        .filter(|i| i.kind == Kind::Marketplace)
        .map(|i| i.group.clone())
        .collect();
    items
        .iter()
        .filter(|i| ids.contains(&i.id) || groups.contains(&i.group))
        .cloned()
        .collect()
}

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `tree::build` plus each row's info-modal text. Last-use data is optional: if an agent's
/// records cannot be read the modal omits it, after a warning.
pub(super) fn display_tree(deps: &Deps, env: &Env, items: &[Installed]) -> Vec<tree::Node> {
    let usage = usage::read(&deps.adapters, env)
        .inspect_err(|e| eprintln!("warning: last-use data unavailable: {e:#}"))
        .ok();
    let now = now_ms();
    let mut nodes = tree::build(items);
    tree::attach_info(&mut nodes, &|i| {
        let item = &items[i];
        let used = match (&usage, prune::tracked(item, &deps.adapters)) {
            (Some(u), true) => u.last_used(item).map_or(info::Used::Never, info::Used::At),
            _ => info::Used::NotTracked,
        };
        info::item_lines(item, used, now)
    });
    nodes
}

/// Browse `items` in the tree UI, then show or remove whatever the user marks.
fn browse(scene: &Scene, items: &[Installed], out: &mut dyn Write) -> anyhow::Result<()> {
    let (deps, env) = (scene.deps, scene.env);
    let nodes = display_tree(deps, env, items);
    let Some((choice, picked)) = deps.prompter.browse("skillz", &nodes)? else {
        return Ok(());
    };
    let chosen: Vec<&Installed> = picked
        .item_indices(&nodes)
        .into_iter()
        .filter_map(|i| items.get(i))
        .collect();
    match choice {
        Choice::Show => {
            write!(
                out,
                "{}",
                tree::render(&tree::build(&queried(items, &chosen)))
            )?;
            Ok(())
        }
        Choice::Remove => {
            let review = Review {
                dry_run: false,
                list_items: true,
            };
            confirm_and_remove(scene, &chosen, review, out)
        }
    }
}

pub(super) fn list(scene: &Scene, view: ListView, out: &mut dyn Write) -> anyhow::Result<()> {
    let deps = scene.deps;
    let mut items = scene.gather();
    if let Some(q) = view.query {
        items = queried(&items, &remove::matches(&items, Some(q)));
    }
    if view.json {
        return print_json(scene, &items, view.check, out);
    }
    if deps.interactive && view.interactive {
        return browse(scene, &items, out);
    }
    render_listing(&listing(&items, &view), out)
}

/// The items and their removal plans, with each one's context cost.
fn print_json(
    scene: &Scene,
    items: &[Installed],
    check: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    // Read the costs only here: the report takes a second or so (`claude -p /context`).
    let costs = cost::read(&scene.deps.adapters, scene.env, scene.deps.runner);
    let rows: Vec<serde_json::Value> = items
        .iter()
        .map(|i| {
            let mut row = serde_json::to_value(i)?;
            row["children"] = serde_json::json!(mcp::shown_children(i));
            row["tokens"] = serde_json::json!(costs.tokens(i));
            row["location"] = serde_json::json!(location(i));
            row["package"] = serde_json::json!(Package::of(i).title());
            row["merged"] = serde_json::json!(!i.members.is_empty());
            row["members"] = members_json(i);
            if check {
                row["update_available"] = serde_json::json!(scene.deps.checker.available(i));
            }
            anyhow::Ok(row)
        })
        .collect::<Result<_, _>>()?;
    serde_json::to_writer_pretty(&mut *out, &rows)?;
    writeln!(out)?;
    Ok(())
}

/// The per-agent copies a merged item stands for, each with its own id, agents and location.
fn members_json(item: &Installed) -> serde_json::Value {
    let copies = item
        .members
        .iter()
        .map(|m| serde_json::json!({ "id": m.id, "agents": m.agents, "location": location(m) }));
    copies.collect()
}

/// What `list` prints for a plain (non-json, non-interactive) run.
#[derive(Debug, PartialEq, Eq)]
struct Listing {
    empty: Option<Empty>,
    nodes: Vec<tree::Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Empty {
    NothingInstalled,
    NoMatch,
}

fn listing(items: &[Installed], view: &ListView) -> Listing {
    let empty = match (items.is_empty(), view.query.is_some()) {
        (false, _) => None,
        (true, true) => Some(Empty::NoMatch),
        (true, false) => Some(Empty::NothingInstalled),
    };
    let nodes = tree::build(items);
    let expand = view.all || view.query.is_some();
    let nodes = if expand { nodes } else { tree::collapse(nodes) };
    Listing { empty, nodes }
}

fn render_listing(listing: &Listing, out: &mut dyn Write) -> anyhow::Result<()> {
    match listing.empty {
        Some(Empty::NoMatch) => writeln!(out, "nothing installed matches")?,
        Some(Empty::NothingInstalled) => writeln!(out, "nothing installed")?,
        None => {}
    }
    write!(out, "{}", tree::render(&listing.nodes))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::scope::project_env;
    use super::*;
    use crate::harness::agents::Harness;
    use crate::harness::model::{Agent, Installer, Scope};
    use crate::install::test_support::{FakeRunner, ScriptedPrompter};

    fn skill(name: &str) -> Installed {
        Installed {
            details: Default::default(),
            hooks: Vec::new(),
            members: Vec::new(),
            id: name.into(),
            name: name.into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            group: "g".into(),
            source: None,
            installer: Installer::Local,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    struct BrokenUsage;

    impl Harness for BrokenUsage {
        fn agent(&self) -> Agent {
            Agent::Claude
        }
        fn hook_files(&self, _: &Env, _: Scope) -> Vec<std::path::PathBuf> {
            vec![]
        }
        fn installed(&self, _: &Env, _: Scope) -> anyhow::Result<Vec<Installed>> {
            Ok(vec![])
        }
        fn home_dir_names(&self) -> &'static [&'static str] {
            &[]
        }
        fn usage(&self, _: &Env) -> anyhow::Result<Vec<(String, u64)>> {
            anyhow::bail!("unreadable")
        }
    }

    #[test]
    fn unreadable_usage_data_still_leaves_the_tree() {
        let dir = tempfile::tempdir().unwrap();
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let deps = Deps::for_test(
            dir.path(),
            vec![Box::new(BrokenUsage)],
            (&runner, &prompter),
        );
        let env = project_env(&deps, &deps.cwd);

        let nodes = display_tree(&deps, &env, &[skill("a")]);

        // The failed usage read degrades to a warning; the item row is still built.
        assert_eq!(nodes[0].children.len(), 1);
    }

    fn view(query: Option<&str>, all: bool) -> ListView<'_> {
        ListView {
            query,
            all,
            json: false,
            check: false,
            interactive: false,
        }
    }

    #[test]
    fn an_empty_listing_says_whether_a_query_was_given() {
        let plain = listing(&[], &view(None, false));
        let queried = listing(&[], &view(Some("x"), false));

        // Assert the two empty cases are told apart by value.
        assert_eq!(plain.empty, Some(Empty::NothingInstalled));
        assert_eq!(queried.empty, Some(Empty::NoMatch));
    }

    #[test]
    fn all_or_a_query_expands_the_groups() {
        let items = [skill("a")];

        let collapsed = listing(&items, &view(None, false));
        let all = listing(&items, &view(None, true));
        let by_query = listing(&items, &view(Some("a"), false));

        // Assert only a collapsed listing hides the rows under the group header.
        assert!(collapsed.nodes[0].children.is_empty());
        assert_eq!(all, by_query);
        assert_eq!(all.nodes[0].children.len(), 1);
        assert_eq!(all.empty, None);
    }
}
