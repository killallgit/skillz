use std::path::{Path, PathBuf};

use anyhow::Context;

use super::edit::{FilePlan, plan_edits};
use super::json::json_has;
use super::removal_target::remove_path;
use super::stage::{commit, discard, stage};
use super::{Action, RemovalTarget, Report};
use crate::runner::CommandRunner;

/// Dedupe, run every `Run` in plan order (stopping at the first failure), then plan every
/// edit in memory (any failure returns before the filesystem is touched), then write all
/// edited files or none, then remove paths in their original order (checking each
/// `RemovePathIfJsonAbsent` condition against the written files).
pub fn execute(actions: &[Action], runner: &dyn CommandRunner) -> anyhow::Result<Report> {
    execute_with(actions, runner, &mut |_| {})
}

/// `execute`, calling `on_step` before each step so a caller can show progress. Commands run
/// quietly: their own output would scribble over a status line.
pub fn execute_with(
    actions: &[Action],
    runner: &dyn CommandRunner,
    on_step: &mut dyn FnMut(&Action),
) -> anyhow::Result<Report> {
    let unique = unique_actions(actions);
    let mut execution = Execution {
        runner,
        on_step,
        report: Report::default(),
    };
    // Native tools first: they may already have edited the files the later steps clean up
    // (an edit of an absent key is a no-op), and a failure here leaves everything else alone.
    execution.run_commands(&unique)?;
    execution.edit_files(&unique)?;
    execution.remove_paths(&unique)?;
    Ok(execution.report)
}

fn unique_actions(actions: &[Action]) -> Vec<&Action> {
    let mut unique: Vec<&Action> = Vec::new();
    for action in actions {
        if !unique.contains(&action) {
            unique.push(action);
        }
    }
    unique
}

struct Execution<'a> {
    runner: &'a dyn CommandRunner,
    on_step: &'a mut dyn FnMut(&Action),
    report: Report,
}

/// The fields of an `Action::Run`.
struct Command<'a> {
    program: &'a str,
    args: &'a [String],
    cwd: &'a Path,
    fallback: bool,
}

impl<'a> Command<'a> {
    fn of(action: &'a Action) -> Option<Self> {
        match action {
            Action::Run {
                program,
                args,
                cwd,
                fallback,
            } => Some(Self {
                program,
                args,
                cwd,
                fallback: *fallback,
            }),
            _ => None,
        }
    }

    fn line(&self) -> String {
        [self.program]
            .into_iter()
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Execution<'_> {
    fn run_commands(&mut self, actions: &[&Action]) -> anyhow::Result<()> {
        for action in actions {
            if let Some(command) = Command::of(action) {
                (self.on_step)(action);
                self.run_command(&command)?;
            } else if let Action::CopyAgents { .. } = action {
                (self.on_step)(action);
                self.copy_agents(action)?;
            }
        }
        Ok(())
    }

    fn copy_agents(&mut self, action: &Action) -> anyhow::Result<()> {
        let Action::CopyAgents {
            source,
            cwd,
            names,
            ext,
            to,
        } = action
        else {
            return Ok(());
        };
        let request = crate::subagents::Copy {
            source,
            cwd,
            names,
            ext,
            to,
        };
        let written = crate::subagents::copy(self.runner, &request)
            .with_context(|| format!("already ran: {:?}", self.report.ran))?;
        self.report.copied.extend(written);
        Ok(())
    }

    fn run_command(&mut self, command: &Command) -> anyhow::Result<()> {
        let line = command.line();
        let out = match self
            .runner
            .run_quiet(command.program, command.args, command.cwd)
        {
            Ok(out) => out,
            Err(_) if command.fallback => {
                self.report.skipped.push(line);
                return Ok(());
            }
            Err(e) => return Err(e.context(format!("already ran: {:?}", self.report.ran))),
        };
        if !out.success {
            anyhow::bail!("`{line}` failed: {}", out.stderr.trim());
        }
        self.report.ran.push(line);
        Ok(())
    }

    fn edit_files(&mut self, actions: &[&Action]) -> anyhow::Result<()> {
        let plans = plan_edits(actions, &mut *self.on_step)?;
        let edited = write_plans(&plans)?;
        self.report.edited.extend(edited);
        Ok(())
    }

    fn remove_paths(&mut self, actions: &[&Action]) -> anyhow::Result<()> {
        for action in actions {
            let Some(target) = removal_target(action) else {
                continue;
            };
            (self.on_step)(action);
            let removed = remove_if_wanted(action, target)
                .with_context(|| format!("already removed: {:?}", self.report.removed))?;
            if removed {
                self.report.removed.push(target.path().to_path_buf());
            }
        }
        Ok(())
    }
}

/// Stage every changed file, then rename them all into place; on a staging failure the
/// temp files are dropped and nothing is written. Returns the files written.
fn write_plans(plans: &[FilePlan]) -> anyhow::Result<Vec<PathBuf>> {
    let mut staged = Vec::new();
    let mut edited = Vec::new();
    for plan in plans {
        let Some((before, planned)) = plan.change() else {
            continue;
        };
        match stage(&plan.file, before.to_string(), planned) {
            Ok(s) => staged.push(s),
            Err(e) => {
                discard(&staged);
                return Err(e.context(format!("write {}", plan.file.display())));
            }
        }
        edited.push(plan.file.clone());
    }
    commit(&staged)?;
    Ok(edited)
}

fn removal_target(action: &Action) -> Option<&RemovalTarget> {
    match action {
        Action::RemovePath(target) | Action::RemovePathIfJsonAbsent { target, .. } => Some(target),
        Action::Edit { .. } | Action::Run { .. } | Action::CopyAgents { .. } => None,
    }
}

fn remove_if_wanted(action: &Action, target: &RemovalTarget) -> anyhow::Result<bool> {
    if should_remove(action)? {
        remove_path(target)
    } else {
        Ok(false)
    }
}

/// A conditional removal waits until the JSON key it guards is gone.
fn should_remove(action: &Action) -> anyhow::Result<bool> {
    match action {
        Action::RemovePathIfJsonAbsent { file, key, .. } => json_has(file, key).map(|has| !has),
        _ => Ok(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Edit;
    use crate::action::testutil::*;
    use crate::runner::NoCommands;
    use serde_json::{Value, json};
    use std::fs;

    #[test]
    fn execute_removes_conditional_path_only_once_key_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let index = dir.path().join("index.json");
        write(&index, r#"{"plugins": {"a@m": [1], "b@m": [2]}}"#);
        let dirs = ["a", "b", "c"].map(|d| {
            fs::create_dir(dir.path().join(d)).unwrap();
            dir.path().join(d)
        });
        let unless = |path: &Path, file: &Path, id: &str| Action::RemovePathIfJsonAbsent {
            target: RemovalTarget::new(path.parent().unwrap(), [path.file_name().unwrap()])
                .unwrap(),
            file: file.to_path_buf(),
            key: strings(&["plugins", id]),
        };
        let report = execute(
            &[
                unless(&dirs[0], &index, "a@m"),
                unless(&dirs[1], &index, "b@m"),
                unless(&dirs[2], &dir.path().join("missing.json"), "c@m"),
                Action::Edit {
                    file: index.clone(),
                    edit: Edit::JsonRemoveKey {
                        path: strings(&["plugins", "a@m"]),
                    },
                },
            ],
            &NoCommands,
        )
        .unwrap();
        assert_eq!(report.removed, vec![dirs[0].clone(), dirs[2].clone()]);
        assert!(!dirs[0].exists() && dirs[1].exists() && !dirs[2].exists());

        write(&index, "{");
        assert!(execute(&[unless(&dirs[1], &index, "b@m")], &NoCommands).is_err());
        assert!(dirs[1].exists());
    }

    #[test]
    fn execute_edits_json_and_toml() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("settings.json");
        let config = dir.path().join("config.toml");
        write(
            &settings,
            r#"{"enabledPlugins": {"x@mp": true}, "hooks": {"a": [{"command": "c"}]}}"#,
        );
        write(&config, CODEX);
        let key = Action::Edit {
            file: settings.clone(),
            edit: Edit::JsonRemoveKey {
                path: strings(&["enabledPlugins", "x@mp"]),
            },
        };
        let actions = [
            key.clone(),
            Action::Edit {
                file: settings.clone(),
                edit: hook("a", "c"),
            },
            key,
            Action::Edit {
                file: config.clone(),
                edit: Edit::TomlRemoveKey {
                    path: strings(&["plugins", "x@mp"]),
                },
            },
            Action::Edit {
                file: dir.path().join("absent.json"),
                edit: hook("a", "c"),
            },
        ];
        let report = execute(&actions, &NoCommands).unwrap();
        assert_eq!(report.edited, vec![settings.clone(), config.clone()]);
        assert!(report.removed.is_empty());

        let json: Value = serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(json, json!({"enabledPlugins": {}, "hooks": {}}));
        let toml: toml_edit::DocumentMut = fs::read_to_string(&config).unwrap().parse().unwrap();
        assert!(toml["plugins"].get("x@mp").is_none());
    }

    #[test]
    fn execute_unchanged_edit_is_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let src = "{\"a\":1}";
        write(&file, src);
        let action = Action::Edit {
            file: file.clone(),
            edit: hook("x", "y"),
        };
        assert_eq!(execute(&[action], &NoCommands).unwrap(), Report::default());
        assert_eq!(fs::read_to_string(&file).unwrap(), src);
    }

    #[test]
    fn execute_fails_fast_on_invalid_file() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.json");
        let later = dir.path().join("later");
        write(&bad, "{");
        write(&later, "x");
        let result = execute(
            &[
                Action::Edit {
                    file: bad.clone(),
                    edit: hook("a", "b"),
                },
                rm(&later),
            ],
            &NoCommands,
        );
        assert!(result.is_err());
        assert!(later.exists());
        assert_eq!(fs::read_to_string(&bad).unwrap(), "{");
    }

    #[test]
    fn execute_late_edit_failure_touches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let doomed = dir.path().join("doomed");
        let good = dir.path().join("good.json");
        let bad = dir.path().join("bad.json");
        let src = r#"{"a": 1, "b": 2}"#;
        write(&doomed, "x");
        write(&good, src);
        write(&bad, "{");
        let result = execute(
            &[
                rm(&doomed),
                Action::Edit {
                    file: good.clone(),
                    edit: Edit::JsonRemoveKey {
                        path: strings(&["a"]),
                    },
                },
                Action::Edit {
                    file: bad.clone(),
                    edit: hook("a", "b"),
                },
            ],
            &NoCommands,
        );
        assert!(result.is_err());
        assert!(doomed.exists());
        assert_eq!(fs::read_to_string(&good).unwrap(), src);
        assert_eq!(fs::read_to_string(&bad).unwrap(), "{");
    }

    #[cfg(unix)]
    #[test]
    fn execute_writes_through_symlink_and_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("dotfiles-settings.json");
        let link = dir.path().join("settings.json");
        write(&real, r#"{"a": 1, "b": 2}"#);
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let report = execute(
            &[Action::Edit {
                file: link.clone(),
                edit: Edit::JsonRemoveKey {
                    path: strings(&["a"]),
                },
            }],
            &NoCommands,
        )
        .unwrap();
        assert_eq!(report.edited, vec![link.clone()]);
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let json: Value = serde_json::from_str(&fs::read_to_string(&real).unwrap()).unwrap();
        assert_eq!(json, json!({"b": 2}));
        let mode = fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn execute_merges_edits_to_one_file_through_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("a.json");
        let alias = dir.path().join("b.json");
        write(&real, r#"{"x": 1, "y": 2, "z": 3}"#);
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let remove = |file: &Path, key: &str| Action::Edit {
            file: file.to_path_buf(),
            edit: Edit::JsonRemoveKey {
                path: strings(&[key]),
            },
        };

        execute(&[remove(&real, "x"), remove(&alias, "y")], &NoCommands).unwrap();
        let json: Value = serde_json::from_str(&fs::read_to_string(&real).unwrap()).unwrap();
        assert_eq!(json, json!({"z": 3}));
        assert!(fs::symlink_metadata(&alias).unwrap().is_symlink());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn execute_rejects_format_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let json_named_toml = dir.path().join("x.toml");
        write(&json_named_toml, r#"{"a": {"b": 1}}"#);
        let action = Action::Edit {
            file: json_named_toml.clone(),
            edit: Edit::JsonRemoveKey {
                path: strings(&["a", "b"]),
            },
        };
        assert!(execute(&[action], &NoCommands).is_err());
        assert_eq!(
            fs::read_to_string(&json_named_toml).unwrap(),
            r#"{"a": {"b": 1}}"#
        );
    }

    #[test]
    fn execute_read_error_on_directory() {
        let dir = tempfile::tempdir().unwrap();
        let action = Action::Edit {
            file: dir.path().to_path_buf(),
            edit: hook("a", "b"),
        };
        assert!(execute(&[action], &NoCommands).is_err());
    }

    #[test]
    fn commands_run_once_in_order_before_edits_and_removals() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        fs::write(&file, "x").unwrap();
        let runner = Recorder {
            calls: Default::default(),
            fail: false,
        };
        let report = execute(
            &[
                rm(&file),
                run_action("tool", "a", dir.path()),
                run_action("tool", "b", dir.path()),
                run_action("tool", "a", dir.path()),
            ],
            &runner,
        )
        .unwrap();
        assert_eq!(*runner.calls.borrow(), ["tool a", "tool b"]);
        assert_eq!(report.ran, ["tool a", "tool b"]);
        assert_eq!(report.removed, std::slice::from_ref(&file));
        assert!(!file.exists());
    }

    #[test]
    fn failing_command_stops_the_plan_before_any_file_change() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        fs::write(&file, "x").unwrap();
        let runner = Recorder {
            calls: Default::default(),
            fail: true,
        };
        let plan = [run_action("tool", "a", dir.path()), rm(&file)];
        assert!(execute(&plan, &runner).is_err());
        assert!(file.exists());
        // A plan with a command and no runner for it is an error too.
        assert!(execute(&plan, &NoCommands).is_err());
        assert!(file.exists());
    }

    #[test]
    fn fallback_command_that_cannot_run_is_skipped_and_the_file_steps_still_run() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        fs::write(&file, "x").unwrap();
        let native = Action::native("tool", &["rm", "x"], &["x"], dir.path()).unwrap();
        let report = execute(&[native, rm(&file)], &NoCommands).unwrap();
        assert_eq!(report.skipped, ["tool rm x"]);
        assert!(report.ran.is_empty());
        assert!(!file.exists());
    }

    #[test]
    fn execute_with_reports_every_unique_step_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, "x").unwrap();
        let rm = rm(&file);
        let mut seen = 0;
        execute_with(&[rm.clone(), rm], &NoCommands, &mut |_| seen += 1).unwrap();
        assert_eq!(seen, 1);
    }
}
