#[macro_use]
extern crate pest_derive;

#[macro_use]
extern crate lazy_static;

use std::fs;

mod parser;
mod name_analysis;
mod codegen;

fn main() {
    let unparsed_file = fs::read_to_string("src/samples/test.sp").expect("cannot read file");

    let prog = parser::parse(&unparsed_file).expect("Parse failed");
    println!("{:#?}", prog);

    let analyzed_prog = name_analysis::name_analysis(prog);
    println!("{:#?}", analyzed_prog);

}
