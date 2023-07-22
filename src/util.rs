use crate::*;
use image::{imageops::FilterType, DynamicImage, GenericImageView};
use lz4_flex::frame as lz4;
use std::{fmt::Debug, io};

pub fn purify_err<T, E: Debug>(msg: &str, r: Result<T, E>) -> T {
    return match r {
        Ok(obj) => obj,
        Err(e) => panic!("{}: {:?}", msg, e),
    };
}

pub fn purify_opt<T>(msg: &str, o: Option<T>) -> T {
    return match o {
        Some(o) => o,
        None => panic!("{}", msg),
    };
}

pub fn create_dir<P: AsRef<Path>>(p: P) {
    let p = p.as_ref();
    if !p.exists() {
        purify_err(
            &format!("Failed to create dir \"{}\"", p.to_string_lossy()),
            std::fs::create_dir_all(p),
        );
    } else if !p.is_dir() {
        panic!("\"{}\" already existed but not a dir", p.to_string_lossy())
    }
}

pub fn whether_dump(b: bool, p: &str) -> Option<PathBuf> {
    return match b {
        false => None,
        true => {
            let p = PathBuf::from(p);
            create_dir(&p);
            Some(p)
        }
    };
}

pub fn whether_dir<P: AsRef<Path>>(
    path: P,
    m1: &'static str,
    m2: &'static str,
    verbose: bool,
) -> Box<dyn Iterator<Item = Result<PathBuf, String>>> {
    return Box::new(match std::fs::read_dir(path) {
        Ok(d) => d.into_iter().map(move |d| match d {
            Ok(d) => Ok(d.path()),
            Err(e) => Err(match verbose {
                true => format!("Failed to access {}: {:?}", m2, e),
                false => String::with_capacity(0),
            }),
        }),
        Err(e) => panic!("Failed to access {}: {:?}", m1, e),
    });
}

pub fn lz4read<R: io::Read>(r: R) -> lz4::FrameDecoder<R> {
    return lz4::FrameDecoder::new(r);
}

pub fn lz4write<W: io::Write>(w: W) -> lz4::FrameEncoder<W> {
    return lz4::FrameEncoder::with_frame_info(lz4cfg(), w);
}

pub fn lz4cfg() -> lz4::FrameInfo {
    let mut cfg = lz4::FrameInfo::new();
    cfg.block_mode = lz4::BlockMode::Linked;
    cfg.block_checksums = true;
    return cfg;
}

pub fn img3(
    mut img: DynamicImage,
    crop: Option<(u32, u32, u32, u32)>,
    resize: Option<(u32, u32)>,
    zoom: Option<f32>,
    filter: FilterType,
) -> DynamicImage {
    if let Some((w, h, x, y)) = crop {
        img = img.crop_imm(x, y, w, h);
    }
    if let Some((nw, nh)) = resize {
        img = img.resize(nw, nh, filter);
    } else if let Some(z) = zoom {
        img = img.resize(
            (img.width() as f32 * z) as u32,
            (img.height() as f32 * z) as u32,
            filter,
        );
    }
    return img;
}

#[macro_export]
#[rustfmt::skip]
macro_rules! unsafe_init { () => {{ unsafe { std::mem::MaybeUninit::uninit().assume_init() } }}; }

#[macro_export]
macro_rules! try_again {
    ($func:expr , $msg:literal $(, $args:expr)* $(,)?) => {{
        let v;
        loop {
            v = match $func {
                Ok(v) => v,
                Err(e) => {
                    println!($msg $(, $args)* , e);
                    println!("(press ENTER to try again or press CTRL-C to terminate)");
                    pause!();
                }
            };
            break;
        }
        v
    }};
}

#[macro_export]
#[rustfmt::skip]
macro_rules! pause { () => {{ std::io::stdin().read(&mut [0u8]).unwrap(); }}; }
