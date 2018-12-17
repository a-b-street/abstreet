use crate::Key;
use std::collections::HashSet;

pub struct TopMenu {
    folders: Vec<Folder>,
}

impl TopMenu {
    pub fn new(folders: Vec<Folder>) -> TopMenu {
        let mut keys: HashSet<Key> = HashSet::new();
        for f in &folders {
            for (key, _) in &f.actions {
                if keys.contains(key) {
                    panic!("TopMenu uses {:?} twice", key);
                }
                keys.insert(*key);
            }
        }

        TopMenu { folders }
    }
}

pub struct Folder {
    name: String,
    actions: Vec<(Key, String)>,
}

impl Folder {
    pub fn new(name: &str, actions: Vec<(Key, &str)>) -> Folder {
        Folder {
            name: name.to_string(),
            actions: actions
                .into_iter()
                .map(|(key, action)| (key, action.to_string()))
                .collect(),
        }
    }
}
