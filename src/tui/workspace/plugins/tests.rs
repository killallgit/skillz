use ratatui::crossterm::event::KeyCode;

use super::rows::Plugin;
use super::*;
use crate::app::library::{Catalog, LibraryItem};
use crate::app::workspace::Act;
use crate::tui::theme::Theme;
use crate::tui::widgets::testing::{key, render_to_buffer, row_text};
use skillz_harness::Spec;
use skillz_harness::model::{Details, Installed, Installer, Kind, Scope};

fn installed(name: &str, kind: Kind, group: &str, source: Option<&str>) -> Installed {
    Installed {
        details: Details::default(),
        hooks: vec![],
        members: vec![],
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
        removal: vec![],
    }
}

fn skill(source: &str, name: &str) -> Spec {
    Spec::Skill {
        source: source.into(),
        names: vec![name.into()],
        agents: vec![],
    }
}

fn library(entries: Vec<Spec>) -> Catalog {
    Catalog::from_items(entries.into_iter().map(LibraryItem::from_entry).collect())
}

fn tab(model: Model) -> PluginsTab {
    tab_for(Scope::Project, model)
}

fn tab_for(scope: Scope, model: Model) -> PluginsTab {
    let mut tab = PluginsTab::new(scope);
    tab.load(&Model {
        agents: vec![Agent::Claude, Agent::Codex],
        ..model
    });
    tab
}

fn project(row: &Row) -> &Plugin {
    match row {
        Row::Own(plugin) => plugin,
        other => panic!("not a project row: {other:?}"),
    }
}

fn globally(name: &str, kind: Kind, group: &str, source: Option<&str>) -> Installed {
    Installed {
        id: format!("global-{kind}:{name}"),
        scope: Scope::Global,
        ..installed(name, kind, group, source)
    }
}

fn names(tab: &PluginsTab) -> Vec<String> {
    tab.rows.iter().map(Row::name).collect()
}

fn press(tab: &mut PluginsTab, code: KeyCode) -> Option<Act> {
    tab.handle(key(code))
}

fn type_text(tab: &mut PluginsTab, text: &str) {
    text.chars().for_each(|c| {
        press(tab, KeyCode::Char(c));
    });
}

fn space() -> KeyCode {
    KeyCode::Char(' ')
}

/// Select the package under the cursor, then submit.
fn flip_and_apply(tab: &mut PluginsTab) -> Option<Act> {
    assert_eq!(press(tab, space()), None);
    press(tab, KeyCode::Enter)
}

fn install_of(entries: Vec<Spec>) -> Option<Act> {
    Some(Act::Change {
        scope: Scope::Project,
        install: entries,
        remove: vec![],
    })
}

#[test]
fn one_row_per_package_by_name_with_no_hooks_mcp_or_local_files() {
    let hook = Spec::Hook {
        event: "Stop".into(),
        matcher: None,
        command: "say done".into(),
        agents: vec![],
    };
    let tab = tab(Model {
        library: library(vec![
            skill("zed/skills", "a"),
            skill("Acme/tools", "b"),
            skill("zed/skills", "c"),
            skill("/home/me/skills", "local"),
            hook,
        ]),
        ..Model::default()
    });

    assert_eq!(names(&tab), ["Acme/tools", "zed/skills"]);
}

#[test]
fn a_package_the_project_has_is_installed_and_lists_the_tools_that_have_it() {
    let tab = tab(Model {
        library: library(vec![skill("o/r", "tdd"), skill("x/y", "other")]),
        installed: vec![installed("tdd", Kind::Skill, "o/r", Some("o/r"))],
        ..Model::default()
    });

    let rows: Vec<_> = tab
        .rows
        .iter()
        .map(|row| {
            let plugin = project(row);
            (row.name(), plugin.is_installed(), plugin.tools.clone())
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("o/r".to_string(), true, vec![Agent::Claude]),
            ("x/y".to_string(), false, vec![]),
        ]
    );
}

#[test]
fn something_installed_that_the_library_lacks_still_shows_so_it_can_be_removed() {
    let tab = tab(Model {
        installed: vec![installed("tdd", Kind::Skill, "o/r", Some("o/r"))],
        ..Model::default()
    });

    assert_eq!(names(&tab), ["o/r"]);
    assert!(project(&tab.rows[0]).is_installed());
}

#[test]
fn a_plugin_and_its_marketplace_make_one_row_removed_together() {
    let plugin = Spec::Plugin {
        source: "/abs/mp".into(),
        marketplace: "mp".into(),
        name: "p".into(),
        agents: vec![],
    };
    let mut tab = tab(Model {
        library: library(vec![plugin]),
        installed: vec![
            installed("p@mp", Kind::Plugin, "mp", None),
            installed("mp", Kind::Marketplace, "mp", None),
        ],
        ..Model::default()
    });

    let act = flip_and_apply(&mut tab);

    assert_eq!(names(&tab), ["mp"]);
    assert_eq!(
        act,
        Some(Act::Change {
            scope: Scope::Project,
            install: vec![],
            remove: vec!["plugin:p@mp".into(), "marketplace:mp".into()],
        })
    );
}

#[test]
fn selecting_an_installed_package_and_submitting_removes_what_the_project_has_of_it() {
    let mut tab = tab(Model {
        library: library(vec![skill("o/r", "tdd")]),
        installed: vec![installed("tdd", Kind::Skill, "o/r", Some("o/r"))],
        ..Model::default()
    });

    let act = flip_and_apply(&mut tab);

    assert_eq!(
        act,
        Some(Act::Change {
            scope: Scope::Project,
            install: vec![],
            remove: vec!["skill:tdd".into()],
        })
    );
}

#[test]
fn selecting_a_package_the_project_lacks_and_submitting_installs_it_for_every_tool() {
    let mut tab = tab(Model {
        library: library(vec![skill("o/r", "tdd"), skill("o/r", "grill")]),
        ..Model::default()
    });

    let act = flip_and_apply(&mut tab);

    let both = vec![Agent::Claude, Agent::Codex];
    let with = |name: &str| Spec::Skill {
        source: "o/r".into(),
        names: vec![name.into()],
        agents: both.clone(),
    };
    assert_eq!(act, install_of(vec![with("tdd"), with("grill")]));
}

#[test]
fn an_entry_that_names_its_tools_keeps_to_them() {
    let codex_only = Spec::Skill {
        source: "o/r".into(),
        names: vec!["tdd".into()],
        agents: vec![Agent::Codex],
    };
    let mut tab = tab(Model {
        library: library(vec![codex_only.clone()]),
        ..Model::default()
    });

    let act = flip_and_apply(&mut tab);

    assert_eq!(act, install_of(vec![codex_only]));
}

#[test]
fn j_and_k_move_between_packages() {
    let mut tab = tab(Model {
        library: library(vec![skill("a/a", "x"), skill("b/b", "y")]),
        ..Model::default()
    });

    press(&mut tab, KeyCode::Char('j'));
    let second = tab.outline.current();
    press(&mut tab, KeyCode::Char('k'));

    assert_eq!((second, tab.outline.current()), (Some(1), Some(0)));
}

#[test]
fn an_empty_list_does_nothing_on_space() {
    let mut tab = tab(Model::default());

    assert_eq!(press(&mut tab, space()), None);
}

#[test]
fn the_filter_takes_the_keys_narrows_the_list_and_escape_clears_it() {
    let mut tab = tab(Model {
        library: library(vec![skill("a/alpha", "x"), skill("b/beta", "y")]),
        ..Model::default()
    });

    press(&mut tab, KeyCode::Char('/'));
    let capturing = tab.captures_keys();
    type_text(&mut tab, "bet");
    press(&mut tab, KeyCode::Enter);
    let narrowed = tab.outline.visible().len();
    let act = flip_and_apply(&mut tab);
    press(&mut tab, KeyCode::Esc);
    press(&mut tab, KeyCode::Esc);

    assert!(capturing && !tab.captures_keys());
    assert_eq!(narrowed, 1);
    assert!(matches!(act, Some(Act::Change { .. })));
    assert_eq!(tab.outline.visible().len(), 2);
}

#[test]
fn a_line_is_a_checkbox_and_a_name_with_the_tools_only_while_installed() {
    let mut tab = tab(Model {
        library: library(vec![skill("o/r", "tdd"), skill("x/y", "other")]),
        installed: vec![installed("tdd", Kind::Skill, "o/r", Some("o/r"))],
        ..Model::default()
    });
    let theme = Theme::dark();

    let buffer = render_to_buffer(40, 4, |f| tab.draw(f, f.area(), &theme));

    // The list's cursor gutter takes the first two cells of every line.
    let past_gutter = |y| row_text(&buffer, y).chars().skip(2).collect::<String>();
    let (on, off) = (past_gutter(0), past_gutter(1));
    assert!(on.starts_with(theme.glyphs.checked));
    assert!(on.contains("o/r") && on.contains("[claude]"));
    assert!(off.starts_with(theme.glyphs.unchecked));
    assert!(off.contains("x/y") && !off.contains('['));
}

#[test]
fn the_first_row_is_a_package_not_a_header() {
    let tab = tab(Model {
        library: library(vec![skill("o/r", "tdd")]),
        ..Model::default()
    });

    assert_eq!(names(&tab), ["o/r"]);
}

fn with_global() -> PluginsTab {
    tab(Model {
        library: library(vec![
            skill("a/mine", "x"),
            skill("b/user-level", "y"),
            skill("c/both", "z"),
        ]),
        installed: vec![
            installed("x", Kind::Skill, "a/mine", Some("a/mine")),
            installed("z", Kind::Skill, "c/both", Some("c/both")),
        ],
        global: vec![
            globally("y", Kind::Skill, "b/user-level", Some("b/user-level")),
            globally("z", Kind::Skill, "c/both", Some("c/both")),
        ],
        ..Model::default()
    })
}

#[test]
fn the_users_packages_come_after_a_gap_and_a_divider_and_leave_the_project_list() {
    let tab = with_global();

    let kinds: Vec<_> = tab
        .rows
        .iter()
        .map(|row| match row {
            Row::Own(_) => "project",
            Row::Gap => "gap",
            Row::Divider => "divider",
            Row::Global(_) => "global",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "project", "project", "gap", "divider", "gap", "global", "global"
        ]
    );
    // `b/user-level` is in effect already, so only the global section lists it.
    let named: Vec<_> = names(&tab).into_iter().filter(|n| !n.is_empty()).collect();
    assert_eq!(
        named,
        ["a/mine", "c/both", "global", "b/user-level", "c/both"]
    );
}

#[test]
fn no_gap_above_the_divider_when_the_project_has_nothing_to_set_it_apart_from() {
    let tab = tab(Model {
        global: vec![globally("y", Kind::Skill, "b/u", Some("b/u"))],
        ..Model::default()
    });

    assert!(matches!(tab.rows[0], Row::Divider));
    assert_eq!(tab.outline.current(), Some(2));
}

#[test]
fn no_global_section_or_divider_without_user_installs() {
    let tab = tab(Model {
        library: library(vec![skill("o/r", "tdd")]),
        ..Model::default()
    });

    assert!(tab.rows.iter().all(|row| matches!(row, Row::Own(_))));
}

#[test]
fn the_cursor_steps_over_the_gap_and_divider_both_ways() {
    let mut tab = with_global();
    press(&mut tab, KeyCode::Char('j'));

    press(&mut tab, KeyCode::Char('j'));
    let down = tab.outline.current();
    press(&mut tab, KeyCode::Char('k'));
    let up = tab.outline.current();
    press(&mut tab, KeyCode::Char('G'));
    let bottom = tab.outline.current();

    assert_eq!((down, up, bottom), (Some(5), Some(1), Some(6)));
}

#[test]
fn the_users_packages_cannot_be_changed() {
    let mut tab = with_global();
    tab.outline.select_row(5);

    let by_space = press(&mut tab, space());
    let by_enter = press(&mut tab, KeyCode::Enter);

    assert_eq!((by_space, by_enter), (None, None));
}

#[test]
fn the_label_sits_in_the_middle_of_the_rule_in_the_same_dim_as_the_tools() {
    let mut tab = with_global();
    let theme = Theme::dark();

    let buffer = render_to_buffer(40, 8, |f| tab.draw(f, f.area(), &theme));

    let divider: String = row_text(&buffer, 3).chars().skip(2).collect();
    let rule = theme.glyphs.rule.chars().next().unwrap();
    let before = divider.chars().take_while(|&c| c == rule).count();
    let after = divider.chars().rev().take_while(|&c| c == rule).count();
    assert!(divider.contains(" global "));
    assert!(
        before.abs_diff(after) <= 1,
        "{before} rule cells, then {after}"
    );
    assert_eq!(buffer[(10, 3)].style().fg, theme.muted.fg);
    // A blank line on each side of the divider.
    assert!(row_text(&buffer, 2).trim().is_empty());
    assert!(row_text(&buffer, 4).trim().is_empty());
    assert!(row_text(&buffer, 5).contains("b/user-level"));
}

#[test]
fn the_users_lines_are_dimmer_than_the_projects_but_not_as_dim_as_inactive() {
    let mut tab = with_global();
    let theme = Theme::dark();

    let buffer = render_to_buffer(40, 8, |f| tab.draw(f, f.area(), &theme));

    let style_at = |y: u16| buffer[(4, y)].style().fg;
    // Row 0 has the cursor, so the project's line to compare is row 1.
    assert_ne!(style_at(1), theme.muted.fg);
    assert_eq!(style_at(5), theme.muted.fg);
    assert_ne!(theme.muted, theme.inactive);
}

fn user_level() -> PluginsTab {
    tab_for(
        Scope::Global,
        Model {
            library: library(vec![skill("a/mine", "x"), skill("b/user-level", "y")]),
            installed: vec![installed("x", Kind::Skill, "a/mine", Some("a/mine"))],
            global: vec![globally(
                "y",
                Kind::Skill,
                "b/user-level",
                Some("b/user-level"),
            )],
            ..Model::default()
        },
    )
}

#[test]
fn the_users_list_has_only_user_level_state_with_no_divider_and_nothing_of_the_project() {
    let tab = user_level();

    let rows: Vec<_> = tab
        .rows
        .iter()
        .map(|row| match row {
            Row::Own(plugin) => (plugin.name(), plugin.is_installed()),
            other => panic!("only the user's own rows belong here: {other:?}"),
        })
        .collect();
    // `a/mine` is installed in the project, not at user level, so it shows unchecked.
    assert_eq!(
        rows,
        [
            ("a/mine".to_string(), false),
            ("b/user-level".to_string(), true)
        ]
    );
}

#[test]
fn changes_staged_in_the_users_list_are_applied_at_user_level() {
    let mut tab = user_level();

    press(&mut tab, space());
    press(&mut tab, KeyCode::Char('j'));
    press(&mut tab, space());
    let act = press(&mut tab, KeyCode::Enter);

    let both = vec![Agent::Claude, Agent::Codex];
    assert_eq!(
        act,
        Some(Act::Change {
            scope: Scope::Global,
            install: vec![Spec::Skill {
                source: "a/mine".into(),
                names: vec!["x".into()],
                agents: both,
            }],
            remove: vec!["global-skill:y".into()],
        })
    );
}

#[test]
fn space_only_selects_and_enter_alone_with_nothing_selected_does_nothing() {
    let mut tab = tab(Model {
        library: library(vec![skill("o/r", "tdd")]),
        ..Model::default()
    });

    let nothing = press(&mut tab, KeyCode::Enter);
    let selecting = press(&mut tab, space());

    assert_eq!((nothing, selecting), (None, None));
    assert!(tab.pending.len() == 1);
}

#[test]
fn space_again_deselects_and_escape_drops_every_selection() {
    let mut tab = tab(Model {
        library: library(vec![skill("a/a", "x"), skill("b/b", "y")]),
        ..Model::default()
    });

    press(&mut tab, space());
    press(&mut tab, space());
    let after_toggling_twice = tab.pending.len();
    press(&mut tab, space());
    press(&mut tab, KeyCode::Char('j'));
    press(&mut tab, space());
    let selected = tab.pending.len();
    press(&mut tab, KeyCode::Esc);

    assert_eq!(
        (after_toggling_twice, selected, tab.pending.len()),
        (0, 2, 0)
    );
    assert_eq!(press(&mut tab, KeyCode::Enter), None);
}

#[test]
fn installs_and_removals_selected_together_are_one_change() {
    let mut tab = tab(Model {
        library: library(vec![skill("a/new", "x"), skill("b/old", "y")]),
        installed: vec![installed("y", Kind::Skill, "b/old", Some("b/old"))],
        ..Model::default()
    });

    press(&mut tab, space());
    press(&mut tab, KeyCode::Char('j'));
    press(&mut tab, space());
    let act = press(&mut tab, KeyCode::Enter);

    let both = vec![Agent::Claude, Agent::Codex];
    assert_eq!(
        act,
        Some(Act::Change {
            scope: Scope::Project,
            install: vec![Spec::Skill {
                source: "a/new".into(),
                names: vec!["x".into()],
                agents: both,
            }],
            remove: vec!["skill:y".into()],
        })
    );
}

#[test]
fn a_selection_stays_while_the_package_is_as_it_was_and_goes_once_it_changed() {
    let before = || Model {
        library: library(vec![skill("o/r", "tdd")]),
        ..Model::default()
    };
    let mut tab = tab(before());
    press(&mut tab, space());

    // A cancelled change reloads the same state: the selection is kept.
    tab.load(&before());
    let kept = tab.pending.len();
    // An applied one reloads with the package installed: nothing is left to apply.
    tab.load(&Model {
        installed: vec![installed("tdd", Kind::Skill, "o/r", Some("o/r"))],
        ..before()
    });

    assert_eq!((kept, tab.pending.len()), (1, 0));
}

#[test]
fn a_selected_line_is_bold_and_shows_the_state_it_will_have() {
    let mut tab = tab(Model {
        library: library(vec![skill("a/a", "x"), skill("b/b", "y")]),
        installed: vec![installed("y", Kind::Skill, "b/b", Some("b/b"))],
        ..Model::default()
    });
    let theme = Theme::dark();
    press(&mut tab, space());
    press(&mut tab, KeyCode::Char('j'));
    press(&mut tab, space());
    press(&mut tab, KeyCode::Char('j'));

    let buffer = render_to_buffer(40, 4, |f| tab.draw(f, f.area(), &theme));

    let past_gutter = |y| row_text(&buffer, y).chars().skip(2).collect::<String>();
    // a/a was unchecked and will be installed; b/b was checked and will be removed.
    assert!(past_gutter(0).starts_with(theme.glyphs.checked));
    assert!(past_gutter(1).starts_with(theme.glyphs.unchecked));
    let bold = |y: u16| {
        buffer[(4, y)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    };
    assert!(bold(0) && bold(1));
}

fn with_provenance() -> PluginsTab {
    let mut item = LibraryItem::from_entry(skill("o/r", "tdd"));
    item.provenance = Some(crate::app::library::Provenance {
        link: Some("https://example.test/o/r".into()),
        engine: skillz_harness::model::Installer::Npx,
        version: Some("9.8.7".into()),
        recorded_at: crate::app::library::now_secs(),
        detected: true,
    });
    let mut held = installed("tdd", Kind::Skill, "o/r", Some("o/r"));
    held.details = Details::Skill {
        directory: "/home/me/.claude/skills/tdd".into(),
        install_name: "tdd".into(),
    };
    tab(Model {
        library: Catalog::from_items(vec![item]),
        installed: vec![held],
        ..Model::default()
    })
}

fn info_text(tab: &mut PluginsTab) -> String {
    tab.info
        .as_ref()
        .map(|i| i.lines.join("\n"))
        .unwrap_or_default()
}

#[test]
fn i_opens_an_info_modal_with_the_origin_the_provenance_and_where_it_lives() {
    let mut tab = with_provenance();

    press(&mut tab, KeyCode::Char('i'));
    let text = info_text(&mut tab);

    assert!(tab.captures_keys());
    for fact in [
        "o/r",
        "https://example.test/o/r",
        "9.8.7",
        "/home/me/.claude/skills/tdd",
        "tdd",
    ] {
        assert!(text.contains(fact), "{fact} missing from:\n{text}");
    }
}

#[test]
fn the_info_modal_closes_on_escape_enter_or_i_and_keeps_the_selection_untouched() {
    let mut tab = with_provenance();

    let closed: Vec<bool> = [
        KeyCode::Esc,
        KeyCode::Enter,
        KeyCode::Char('i'),
        KeyCode::Char('q'),
    ]
    .into_iter()
    .map(|code| {
        press(&mut tab, KeyCode::Char('i'));
        let act = press(&mut tab, code);
        act.is_none() && tab.info.is_none()
    })
    .collect();

    assert_eq!(closed, [true; 4]);
    // Enter inside the modal closed it; it did not submit anything.
    assert!(tab.pending.is_empty());
}

#[test]
fn the_modal_is_drawn_over_the_list_with_its_title_and_an_ok_button() {
    let mut tab = with_provenance();
    let theme = Theme::dark();
    press(&mut tab, KeyCode::Char('i'));

    let buffer = render_to_buffer(80, 24, |f| tab.draw(f, f.area(), &theme));

    let text: String = (0..24)
        .map(|y| row_text(&buffer, y))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("[ OK ]") && text.contains("9.8.7"));
}

#[test]
fn a_package_with_no_record_says_it_was_added_by_hand_and_has_nothing_to_fetch() {
    let mut tab = tab(Model {
        library: library(vec![skill("o/r", "tdd")]),
        ..Model::default()
    });

    press(&mut tab, KeyCode::Char('i'));
    let text = info_text(&mut tab);

    assert!(text.contains("added by hand"));
    assert!(!text.contains("fetch"));
}

#[test]
fn info_works_on_the_users_packages_and_not_on_the_divider() {
    let mut tab = with_global();
    tab.outline.select_row(5);

    press(&mut tab, KeyCode::Char('i'));
    let opened = tab.info.is_some();
    press(&mut tab, KeyCode::Esc);

    assert!(opened && tab.info.is_none());
}

#[test]
fn flipping_a_package_installed_in_both_scopes_removes_only_the_projects_copy() {
    let here = installed("tdd", Kind::Skill, "o/r", Some("o/r"));
    let user = globally("tdd", Kind::Skill, "o/r", Some("o/r"));
    let mut tab = tab(Model {
        installed: vec![here.clone()],
        global: vec![user],
        ..Model::default()
    });

    let act = flip_and_apply(&mut tab);

    assert_eq!(
        act,
        Some(Act::Change {
            scope: Scope::Project,
            install: vec![],
            remove: vec![here.id],
        })
    );
}

fn plugin_entry(source: &str, name: &str) -> Spec {
    Spec::Plugin {
        source: source.into(),
        marketplace: "mp".into(),
        name: name.into(),
        agents: vec![],
    }
}

#[test]
fn removing_one_of_two_plugins_of_a_marketplace_keeps_the_marketplace_and_the_other_plugin() {
    let mut tab = tab(Model {
        library: library(vec![plugin_entry("a/one", "p"), plugin_entry("a/two", "q")]),
        installed: vec![
            installed("p@mp", Kind::Plugin, "mp", None),
            installed("q@mp", Kind::Plugin, "mp", None),
            installed("mp", Kind::Marketplace, "mp", None),
        ],
        ..Model::default()
    });

    let act = flip_and_apply(&mut tab);

    assert_eq!(
        act,
        Some(Act::Change {
            scope: Scope::Project,
            install: vec![],
            remove: vec!["plugin:p@mp".into()],
        })
    );
}

fn rows_removing(rows: &[Row], id: &str) -> usize {
    rows.iter()
        .filter(
            |row| matches!(row, Row::Own(plugin) if plugin.installed.iter().any(|held| held == id)),
        )
        .count()
}

#[test]
fn a_marketplace_whose_plugins_are_listed_in_other_rows_has_no_row_of_its_own() {
    let model = Model {
        library: library(vec![plugin_entry("a/one", "p"), plugin_entry("a/two", "q")]),
        installed: vec![
            installed("p@mp", Kind::Plugin, "mp", None),
            installed("q@mp", Kind::Plugin, "mp", None),
            installed("mp", Kind::Marketplace, "mp", None),
        ],
        ..Model::default()
    };

    let rows = super::rows::build(&model, Scope::Project);

    let rows_removing_it = rows_removing(&rows, "marketplace:mp");
    assert_eq!(rows_removing_it, 0);
}

#[test]
fn a_marketplace_left_without_plugins_still_has_a_row_to_remove_it_from() {
    let model = Model {
        installed: vec![installed("mp", Kind::Marketplace, "mp", None)],
        ..Model::default()
    };

    let rows = super::rows::build(&model, Scope::Project);

    let rows_removing_it = rows_removing(&rows, "marketplace:mp");
    assert_eq!(rows_removing_it, 1);
}

#[test]
fn u_updates_what_the_project_has_of_the_package_under_the_cursor() {
    let held = installed("tdd", Kind::Skill, "o/r", Some("o/r"));
    let mut tab = tab(Model {
        library: library(vec![skill("o/r", "tdd")]),
        installed: vec![held.clone()],
        ..Model::default()
    });

    let act = press(&mut tab, KeyCode::Char('u'));

    assert_eq!(
        act,
        Some(Act::Update {
            scope: Scope::Project,
            ids: vec![held.id],
        })
    );
    assert!(tab.pending.is_empty());
}

#[test]
fn u_does_nothing_on_a_package_that_is_not_installed() {
    let mut tab = tab(Model {
        library: library(vec![skill("o/r", "tdd")]),
        ..Model::default()
    });

    assert_eq!(press(&mut tab, KeyCode::Char('u')), None);
}

#[test]
fn u_leaves_the_users_packages_alone() {
    let mut tab = with_global();
    tab.outline.select_row(5);

    assert_eq!(press(&mut tab, KeyCode::Char('u')), None);
}

#[test]
fn u_at_user_level_updates_in_that_scope() {
    let held = globally("tdd", Kind::Skill, "o/r", Some("o/r"));
    let mut tab = tab_for(
        Scope::Global,
        Model {
            library: library(vec![skill("o/r", "tdd")]),
            global: vec![held.clone()],
            ..Model::default()
        },
    );

    let act = press(&mut tab, KeyCode::Char('u'));

    assert_eq!(
        act,
        Some(Act::Update {
            scope: Scope::Global,
            ids: vec![held.id],
        })
    );
}
