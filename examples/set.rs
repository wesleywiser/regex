extern crate regex;

use regex::RegexSetBuilder;

fn main() {
    let set = RegexSetBuilder::new("abc")
                              .union("bc")
                              .compile().unwrap();
    let m = set.is_match("abc");
    println!("match? {:?}", m);
}
