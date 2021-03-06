#[macro_use]
extern crate pest_derive;

#[macro_use]
extern crate lazy_static;

use std::fs;
use std::env;
use std::path::Path;

mod parser;
mod error;
mod name_analysis;
mod typecheck;
mod codegen;

/// Compilation takes place in four phases: Parsing, Name Analysis, Type
/// Checking, and Code Generation. Parsing and Name Analysis both emit their
/// own IR, Type Checking simply emits a mapping from symbols to types, and
/// Code Generation writes the compiled javascript to a file
fn main() {
    let args: Vec<String> = env::args().collect();
    let spruce_code: &String = &args[1];
    let spruce_path = Path::new(spruce_code);

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let unparsed_file = fs::read_to_string(spruce_path).expect(&format!("Cannot find file {}", spruce_code));
    let files = vec![(prelude.as_str(), String::from("prelude")), (unparsed_file.as_str(), String::from("main"))];

    let (analyzed_prog, environment) = match compile(files.clone()){
        Ok(r) => r,
        Err(e) => {
            println!("{}", e.as_str(&files));
            return;
        }
    };

    let mut out_file = fs::File::create("out.js").expect("failed to create file");
    codegen::gen_prog(&mut out_file, &analyzed_prog, &environment);
}

pub fn compile(files: Vec<(&str, String)>) -> Result<(name_analysis::Prog, typecheck::Environment), error::SpruceErr> {
    let prog = parser::parse(files.clone())?;
    println!("{:#?}", prog);

    let analyzed_prog = name_analysis::name_analysis(prog)?;
    println!("{:#?}", analyzed_prog);

    let environment = typecheck::check_prog(&analyzed_prog)?;
    println!("{}", environment.as_str(&analyzed_prog));

    Ok((analyzed_prog, environment))
}

#[test]
fn test_prelude() {
    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), true);
}

#[test]
fn test_scope() {
    let pass_prog = "
x = 0
f() {
    x
}

g() {
    y = True
    case y {
        True -> y
        False -> y
    }
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (pass_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), true);

    let fail_prog = "
f(b) {
    case b {
        True -> {
            x = 1
            x
        }
        False -> {
            y = x
            y
        }
    }
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (fail_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), false);
}

#[test]
fn test_mut() {
    let pass_prog = "
mut x = 0
f() {
    x := 1
}

g() {
    mut y = True
    case y {
        True -> {
            y := False
        }
        False -> {
            y := True
        }
    }

    y
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (pass_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), true);

    let fail_prog = "
f() {
    x = 1
    x := 2
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (fail_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), false);
}

#[test]
fn test_curry() {
    let pass_prog = "
add(x, y) {
    x + y
}

main() {
    add3 = add@(3, _)
    add3(5)
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (pass_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), true);

    let pass_prog = "
add(x, y) {
    x + y
}

main() {
    myList = Cons(Cons(Nil, 2), 1)
    listMap(myList, add@(3, _))
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (pass_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), true);

    let fail_prog = "
add(x, y) {
    x + y
}

main() {
    myList = Cons(Cons(Nil), 2), 1)
    listMap(myList, add@(True, _))
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (fail_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), false);
}

#[test]
fn test_annotations() {
    let pass_prog = "
type FooBar(a, b) {
    Foo(a)
    Bar(b)
}

func(fb: FooBar(a, Bool)) -> Bool {
    case fb {
        Foo(v) -> True
        Bar(b) -> b
    }
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (pass_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), true);

    let fail_prog = "
type FooBar(a, b) {
    Foo(a)
    Bar(b)
}

func(fb: FooBar(Bool, Int)) -> Bool {
    case fb {
        Foo(v) -> True
        Bar(b) -> b
    }
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (fail_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), false);

    let fail_prog = "
badId(x: a) -> b {
    x
}
";

    let prelude = fs::read_to_string("src/prelude.sp").expect("cannot read prelude");
    let files = vec![(prelude.as_str(), String::from("prelude")), (fail_prog, String::from("Main"))];
    let res = compile(files);
    assert_eq!(res.is_ok(), false);
}
