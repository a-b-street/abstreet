use lazy_static::lazy_static;
use std::sync::Mutex;

// TODO Maybe just use Timer.

lazy_static! {
    static ref NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

pub fn note(msg: String) {
    NOTES.lock().unwrap().push(msg);
}

pub fn dump_notes() {
    let mut notes = NOTES.lock().unwrap();

    // TODO log or println?
    for msg in notes.iter() {
        println!("{}", msg);
    }

    notes.clear();
}
