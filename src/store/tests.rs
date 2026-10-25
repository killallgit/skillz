use std::fs;
use std::thread;

use super::*;
use crate::harness::Spec;
use crate::library::LibraryItem;
use crate::profile::Profile;

fn store_in(dir: &tempfile::TempDir) -> Store {
    Store::new(dir.path().join("skillz/store.json"))
}

fn skill_item(name: &str) -> LibraryItem {
    LibraryItem::from_entry(Spec::Skill {
        source: "o/r".into(),
        names: vec![name.into()],
        agents: vec![],
    })
}

#[test]
fn the_store_lives_in_the_skillz_dir() {
    let env = Env::new("/home/me".into(), "/work".into());

    let store = Store::in_env(&env);

    assert_eq!(
        store.path,
        crate::app_paths::skillz_dir(&env).join("store.json")
    );
}

#[test]
fn a_missing_file_reads_as_an_empty_store() {
    let dir = tempfile::tempdir().unwrap();

    let document = store_in(&dir).read().unwrap();

    assert_eq!(document, Document::default());
}

#[test]
fn an_update_is_read_back_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);

    store
        .update(|d| {
            d.items.add(skill_item("tdd"));
            d.profiles.push(Profile::new("web"));
            Ok(())
        })
        .unwrap();

    let document = store.read().unwrap();
    assert_eq!(document.items.items().len(), 1);
    assert_eq!(document.profiles, [Profile::new("web")]);
}

#[test]
fn a_corrupt_file_is_a_typed_error_and_is_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);
    let file = dir.path().join("skillz/store.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "{ not json").unwrap();

    let read = store.read().unwrap_err();
    let update = store.update(|_| Ok(())).unwrap_err();

    // Assert both fail with Corrupt and the bytes are still there.
    assert!(read.downcast_ref::<Corrupt>().is_some());
    assert!(update.downcast_ref::<Corrupt>().is_some());
    assert_eq!(fs::read_to_string(&file).unwrap(), "{ not json");
}

#[test]
fn a_store_of_another_version_is_corrupt_rather_than_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("skillz/store.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, r#"{"version": 99}"#).unwrap();

    let error = store_in(&dir).read().unwrap_err();

    assert!(error.downcast_ref::<Corrupt>().is_some());
}

#[test]
fn a_failed_change_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(&dir);

    let result: anyhow::Result<()> = store.update(|d| {
        d.profiles.push(Profile::new("web"));
        anyhow::bail!("no")
    });

    assert!(result.is_err());
    assert_eq!(store.read().unwrap(), Document::default());
}

#[test]
fn writers_of_both_sections_at_once_lose_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("skillz/store.json");
    let writers: Vec<_> = (0..8)
        .map(|n| {
            let store = Store::new(path.clone());
            thread::spawn(move || {
                store.update(|d| {
                    match n % 2 {
                        0 => d.items.add(skill_item(&format!("s{n}"))),
                        _ => d.profiles.push(Profile::new(&format!("p{n}"))),
                    }
                    Ok(())
                })
            })
        })
        .collect();

    writers.into_iter().for_each(|w| w.join().unwrap().unwrap());

    let document = Store::new(path).read().unwrap();
    assert_eq!(document.items.items().len(), 4);
    assert_eq!(document.profiles.len(), 4);
}
