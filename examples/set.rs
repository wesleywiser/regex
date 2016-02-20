extern crate regex;

use regex::RegexSetBuilder;

fn main() {
    let set = RegexSetBuilder::new(r"[a-z]+?")
                              .union("abc")
                              .compile().unwrap();
    // let set = RegexSetBuilder::new(r"abc")
                              // .union("[a-z]+")
                              // .compile().unwrap();
    let m = set.is_match("abc");
    println!("match? {:?}", m);
}
