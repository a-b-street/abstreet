extern crate syn;

use std::fs::File;
use std::io::Read;

fn main() {
    run("../sim/src/spawn.rs");
}

fn run(path: &str) {
    let mut file = File::open(path).expect("Unable to open file");

    let mut src = String::new();
    file.read_to_string(&mut src).expect("Unable to read file");

    let ast = syn::parse_file(&src).expect("Unable to parse file");

    for item in &ast.items {
        match item {
            /*syn::Item::Struct(some_struct) => {
                println!("got a struct: {:?}", some_struct);
                println!("struct {:?}", some_struct.ident);
            }*/
            syn::Item::Impl(some_impl) => {
                let impl_name = simple_name_of_type(&some_impl.self_ty);
                println!("impl {} {{", impl_name);

                for member in &some_impl.items {
                    match member {
                        syn::ImplItem::Method(method) => {
                            println!("  fn {}", method.sig.ident.to_string());
                            for arg in &method.sig.decl.inputs {
                                match arg {
                                    syn::FnArg::Captured(capt) => {
                                        let arg_name = match capt.pat {
                                            syn::Pat::Ident(ref ident) => ident.ident.to_string(),
                                            ref x => format!("{:?}", x),
                                        };
                                        println!(
                                            "    arg {} is {}",
                                            arg_name,
                                            simple_name_of_type(&capt.ty)
                                        )
                                    }
                                    _ => {}
                                }
                            }

                            // Now look for function calls in the body
                            for stmt in &method.block.stmts {
                                match stmt {
                                    syn::Stmt::Local(local) => {
                                        if let Some((_, ref expr)) = local.init {
                                            look_for_function_calls(&expr);
                                        }
                                    }
                                    syn::Stmt::Item(_) => {}
                                    syn::Stmt::Expr(expr) => look_for_function_calls(expr),
                                    syn::Stmt::Semi(expr, _) => look_for_function_calls(expr),
                                }
                            }
                        }
                        _ => {}
                    }
                }
                println!("}}\n");
            }
            _ => {}
        }
    }

    //println!("{:#?}", syntax);
}

fn simple_name_of_type(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => simple_name_of_path(&path.path),
        syn::Type::Reference(tyref) => simple_name_of_type(&tyref.elem),
        x => panic!("doh {:?}", x),
    }
}

fn simple_name_of_path(path: &syn::Path) -> String {
    path.segments.first().unwrap().value().ident.to_string()
}

fn simple_name_of_field(field: &syn::ExprField) -> String {
    let base = match *field.base {
        syn::Expr::Path(ref path) => simple_name_of_path(&path.path),
        ref x => format!("{:?}", x),
    };
    let member = match field.member {
        syn::Member::Named(ref ident) => ident.to_string(),
        syn::Member::Unnamed(ref index) => format!("{}", index.index),
    };
    format!("{}.{}", base, member)
}

// TODO tons more recursion needed to find all calls
fn look_for_function_calls(expr: &syn::Expr) {
    match expr {
        syn::Expr::MethodCall(ref call) => {
            let receiver = match *call.receiver {
                syn::Expr::Path(ref path) => simple_name_of_path(&path.path),
                syn::Expr::Field(ref field) => simple_name_of_field(field),
                ref x => format!("{:?}", x),
            };
            println!("      call {} on {}", call.method.to_string(), receiver);
        }
        _ => {}
    }
}

// What's the output from crawling the AST that I want? Vec<Call>

struct Function {
    type_name: String,
    method_name: String,
}

struct Call {
    caller: Function,
    calls: Function,
}

// From this, I want the bottom-up picture:
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
