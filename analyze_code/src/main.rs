extern crate syn;

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;

fn main() {
    let calls = extract_calls_from_file("../sim/src/spawn.rs");
    for c in &calls {
        println!("{} calls {}", c.call_site, c.function_called);
    }
    // The output is currently something like:
    // Spawner.seed_bus_route calls TransitSimState.create_empty_route
    // Spawner.seed_specific_parked_cars calls ParkingSimState.get_all_spots
    // Spawner.ped_finished_bus_ride calls TripManager.ped_finished_bus_ride
    // Spawner.car_reached_parking_spot calls TripManager.car_reached_parking_spot
    // Spawner.ped_reached_parking_spot calls TripManager.ped_reached_parking_spot

    // From this, I'll form the bottom-up picture:
    //
    // TripManager.ped_finished_bus_ride, TripManager.second_fxn_call_with_same_callers
    //   - called by Spawner.ped_finished_bus_ride
    //      - called by TransitSimState.step
    //          - called by Sim.step
    //
    // and the top-down picture:
    // - Sim.step
    //   - calls TransitSimState.step
    //     - calls Spawner.ped_finished_bus_ride
    //       - calls TripManager.ped_finished_bus_ride
}

#[derive(Clone)]
struct Function {
    type_name: String,
    method_name: String,
}

impl Function {
    fn new(type_name: String, method_name: String) -> Function {
        Function {
            type_name,
            method_name,
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}", self.type_name, self.method_name)
    }
}

struct Call {
    call_site: Function,
    function_called: Function,
}

impl Call {
    fn new(call_site: Function, function_called: Function) -> Call {
        Call {
            call_site,
            function_called,
        }
    }
}

fn extract_calls_from_file(path: &str) -> Vec<Call> {
    let mut file = File::open(path).expect("Unable to open file");
    let mut src = String::new();
    file.read_to_string(&mut src).expect("Unable to read file");
    let ast = syn::parse_file(&src).expect("Unable to parse file");

    let mut result: Vec<Call> = Vec::new();
    for item in &ast.items {
        if let syn::Item::Impl(impl_item) = item {
            let impl_name = simple_name_of_type(&impl_item.self_ty);

            for member in &impl_item.items {
                if let syn::ImplItem::Method(method) = member {
                    let call_site = Function::new(impl_name.clone(), method.sig.ident.to_string());
                    result.extend(extract_calls_from_method(call_site, method));
                }
            }
        }
    }
    result
}

type ArgMap = HashMap<String, String>;

fn extract_calls_from_method(call_site: Function, method: &syn::ImplItemMethod) -> Vec<Call> {
    let mut args: ArgMap = HashMap::new();
    for arg in &method.sig.decl.inputs {
        if let syn::FnArg::Captured(capt) = arg {
            let arg_name = match capt.pat {
                syn::Pat::Ident(ref ident) => ident.ident.to_string(),
                ref x => format!("{:?}", x),
            };
            args.insert(arg_name, simple_name_of_type(&capt.ty));
        }
    }

    let mut result: Vec<Call> = Vec::new();

    for stmt in &method.block.stmts {
        match stmt {
            syn::Stmt::Local(local) => {
                if let Some((_, ref expr)) = local.init {
                    result.extend(extract_calls_from_expr(call_site.clone(), &args, &expr));
                }
            }
            syn::Stmt::Item(_) => {}
            syn::Stmt::Expr(expr) => {
                result.extend(extract_calls_from_expr(call_site.clone(), &args, expr))
            }
            syn::Stmt::Semi(expr, _) => {
                result.extend(extract_calls_from_expr(call_site.clone(), &args, expr))
            }
        }
    }

    result
}

fn extract_calls_from_expr(call_site: Function, args: &ArgMap, expr: &syn::Expr) -> Vec<Call> {
    let mut result: Vec<Call> = Vec::new();

    match expr {
        syn::Expr::MethodCall(ref call) => {
            if let syn::Expr::Path(ref path) = *call.receiver {
                let arg_type = args[&simple_name_of_path(&path.path)].clone();
                result.push(Call::new(
                    call_site,
                    Function::new(arg_type, call.method.to_string()),
                ));
            }
        }
        // TODO https://docs.rs/syn/0.14.9/syn/enum.Expr.html
        // There are so many other places to recurse and find possible calls. In ExprCall for
        // example, we'd have to go find the function body and figure out which of our arguments
        // were passed through.
        _ => {}
    }

    result
}

fn simple_name_of_type(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => simple_name_of_path(&path.path),
        syn::Type::Reference(tyref) => simple_name_of_type(&tyref.elem),
        x => panic!("Weird type {:?}", x),
    }
}

fn simple_name_of_path(path: &syn::Path) -> String {
    path.segments.first().unwrap().value().ident.to_string()
}
