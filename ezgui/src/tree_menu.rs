use keys::describe_key;
use piston::input::Key;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

pub struct TreeMenu {
    root: BTreeMap<String, Item>,
}

impl TreeMenu {
    pub fn new() -> TreeMenu {
        TreeMenu {
            root: BTreeMap::new(),
        }
    }

    pub fn add_action(&mut self, hotkey: Option<Key>, path: &str, action: &str) {
        // Split returns something for an empty string
        if path == "" {
            populate_tree(VecDeque::new(), &mut self.root, hotkey, action);
            return;
        }

        let parts: Vec<&str> = path.split("/").collect();
        populate_tree(VecDeque::from(parts), &mut self.root, hotkey, action);
    }
}

enum Item {
    Action(Option<Key>),
    Tree(Option<Key>, BTreeMap<String, Item>),
}

impl fmt::Display for TreeMenu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TreeMenu:\n")?;
        print(0, &self.root, f)
    }
}

fn print(depth: usize, tree: &BTreeMap<String, Item>, f: &mut fmt::Formatter) -> fmt::Result {
    let pad = String::from_utf8(vec![b' '; 4 * depth]).unwrap();
    for (name, item) in tree {
        match item {
            Item::Action(key) => {
                write!(f, "{}- {} ({})\n", pad, name, describe_maybe_key(key))?;
            }
            Item::Tree(key, subtree) => {
                write!(f, "{}- {} ({})\n", pad, name, describe_maybe_key(key))?;
                print(depth + 1, subtree, f)?;
            }
        }
    }
    Ok(())
}

fn describe_maybe_key(key: &Option<Key>) -> String {
    match key {
        Some(k) => describe_key(*k),
        None => "".to_string(),
    }
}

fn populate_tree(
    mut path_parts: VecDeque<&str>,
    tree: &mut BTreeMap<String, Item>,
    hotkey: Option<Key>,
    action: &str,
) {
    let part = match path_parts.pop_front() {
        Some(p) => p,
        None => {
            assert!(!tree.contains_key(action));
            tree.insert(action.to_string(), Item::Action(hotkey));
            return;
        }
    };

    if !tree.contains_key(part) {
        tree.insert(part.to_string(), Item::Tree(None, BTreeMap::new()));
    }

    match tree.get_mut(part).unwrap() {
        Item::Action(_) => {
            panic!("add_action specifies a path that's an action, not a subtree");
        }
        Item::Tree(_, subtree) => {
            populate_tree(path_parts, subtree, hotkey, action);
        }
    }
}
