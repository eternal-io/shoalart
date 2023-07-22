mod algorithm;
mod routine;
mod util;

pub use ahash::{AHashMap, AHashSet};
pub use std::path::{Path, PathBuf};
pub use structopt::StructOpt;

use routine::*;
use rustdct::num_traits::Num;
use std::panic;

////////////////////////////////////////

#[derive(StructOpt, Debug)]
#[structopt(
    // `tprint("Shoalart", font="lean")`
    long_about = "
          _/_/_/  _/                            _/                        _/
       _/        _/_/_/      _/_/      _/_/_/  _/    _/_/_/  _/  _/_/  _/_/_/_/
        _/_/    _/    _/  _/    _/  _/    _/  _/  _/    _/  _/_/        _/
           _/  _/    _/  _/    _/  _/    _/  _/  _/    _/  _/          _/
    _/_/_/    _/    _/    _/_/      _/_/_/  _/    _/_/_/  _/            _/_/

                                  *** NOTICE ***
Linking between subroutines depends on: All files in one specified directory that
sorted in ascending order. Shoalart will not check the number of files and suffixes!",
    after_help = "[ Copyright (C) K--Aethiax 2021-2022 · All rights reserved ]"
)]
enum Opt {
    Charset(charset::Param),
    Edgedet(edgedet::Param),
    Art(art::Param),
}

const INVALID_SYNTAX: &str = "Invalid syntax";
const INVALID_NUMBER: &str = "Invalid number";

#[rustfmt::skip]
fn opt_crop<T: Num>(s: &str) -> Result<(T, T, T, T), &'static str> {
    let p0 =  s.find("x").ok_or(INVALID_SYNTAX)?;
    let p1 =  s.find("+").ok_or(INVALID_SYNTAX)?;
    let p2 = s.rfind("+").ok_or(INVALID_SYNTAX)?;
    return Ok((
        T::from_str_radix(&s[0..p0],           10).ok().ok_or(INVALID_NUMBER)?,
        T::from_str_radix(&s[p0 + 1..p1],      10).ok().ok_or(INVALID_NUMBER)?,
        T::from_str_radix(&s[p1 + 1..p2],      10).ok().ok_or(INVALID_NUMBER)?,
        T::from_str_radix(&s[p2 + 1..s.len()], 10).ok().ok_or(INVALID_NUMBER)?,
    ));
}

#[rustfmt::skip]
fn opt_resize<T: Num>(s: &str) -> Result<(T, T), &'static str> {
    let p = s.find("x").ok_or(INVALID_SYNTAX)?;
    return Ok((
        T::from_str_radix(&s[0..p],           10).ok().ok_or(INVALID_NUMBER)?,
        T::from_str_radix(&s[p + 1..s.len()], 10).ok().ok_or(INVALID_NUMBER)?,
    ));
}

////////////////////////////////////////

fn main() {
    #[cfg(not(debug_assertions))]
    panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<String>() {
            String::from(s)
        } else if let Some(&s) = info.payload().downcast_ref::<&str>() {
            String::from(s)
        } else {
            String::new()
        };
        if msg.is_empty() {
            println!("*** TERMINATED ***");
        } else {
            println!("*** TERMINATION caused by: {} ***", msg);
        }
    }));
    match Opt::from_args() {
        Opt::Charset(param) => charset::main(param),
        Opt::Edgedet(param) => edgedet::main(param),
        Opt::Art(param) => art::main(param),
    }
    println!("*** DONE ***");
}
