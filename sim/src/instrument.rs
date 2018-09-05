use backtrace::Backtrace;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug)]
struct CallStack {
    calls: Vec<String>,
}

lazy_static! {
    static ref BACKTRACES: Mutex<HashMap<String, CallStack>> = Mutex::new(HashMap::new());
}

pub fn capture_backtrace() {
    let bt = Backtrace::new();
    let mut found_this_fxn = false;
    let mut calls: Vec<String> = Vec::new();
    for f in bt.frames() {
        let raw_name = format!("{}", f.symbols()[0].name().unwrap());
        let mut raw_name_parts: Vec<&str> = raw_name.split("::").collect();
        raw_name_parts.pop();
        let name = raw_name_parts.join("::");

        if found_this_fxn {
            calls.push(name.to_string());
            if name == "sim::sim::Sim::inner_step" {
                break;
            }
        } else {
            if name.ends_with("::capture_backtrace") {
                found_this_fxn = true;
            }
        }
    }

    let caller = &calls[0];
    let stack = CallStack {
        calls: calls[1..].to_vec(),
    };
    println!("insert {}: {:?}", caller, stack);
    let mut remember = BACKTRACES.lock().unwrap();
    remember.insert(caller.to_string(), stack);
}

// TODO dump to file
// TODO manually call when events are created and at other interesting points
// TODO compiler flag so capture_backtrace is usually a no-op
