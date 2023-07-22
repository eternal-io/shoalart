use crate::*;
use edge_detection::canny;
use image::{imageops::Lanczos3, DynamicImage, Luma};
use std::{
    io::{stdout, Write},
    time::Instant,
};

/// Use Canny detect edges for images
///
/// Parallel acceleration is enabled by default!
#[derive(StructOpt, Debug)]
pub struct Param {
    #[structopt(parse(from_os_str))]
    image_dir_or_file: PathBuf,
    #[structopt(parse(from_os_str))]
    output_dir_or_file: PathBuf,

    /// Set the sigma
    #[structopt(short = "s", long, default_value = "2.35")]
    sigma: f32,
    /// Set the strong threshold
    #[structopt(short = "S", long = "strong", default_value = "0.18")]
    thr_strong: f32,
    /// Set the weak threshold
    #[structopt(short = "w", long = "weak", default_value = "0.08")]
    thr_weak: f32,

    /// Crop images before resize; No cropping by default
    ///
    /// Syntax: `{width}x{height}+{left}+{top}` (unit: px; Positive numbers only)
    #[structopt(long, parse(try_from_str = opt_crop))]
    crop: Option<(u32, u32, u32, u32)>,
    /// Resize images before process; No resizing by default
    ///
    /// Syntax: `{nwidth}x{nheight}` (unit: px; Positive numbers only)
    #[structopt(long, parse(try_from_str = opt_resize))]
    resize: Option<(u32, u32)>,
    /// Conflicted with `resize`, but proportionally; Float
    #[structopt(short, long)]
    zoom: Option<f32>,

    /// Specify the value of skipping first N INPUT files
    #[structopt(long = "skip", default_value = "0")]
    i_skip: usize,
    /// Sepcify the step of skipping INPUT files; Used for peek results
    #[structopt(long = "step", default_value = "1")]
    i_step: usize,
    /// Specify the start value of OUTPUT filename
    #[structopt(long = "ctr", default_value = "1")]
    i_ctr: u32,

    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,
}

pub fn main(
    Param {
        image_dir_or_file,
        output_dir_or_file,
        sigma,
        thr_weak,
        thr_strong,
        crop,
        resize,
        zoom,
        i_skip,
        i_step,
        i_ctr,
        verbose,
    }: Param,
) {
    let verbose = verbose > 0;
    let srcs: Box<dyn Iterator<Item = Result<PathBuf, String>>>;
    let dsts: Box<dyn Iterator<Item = PathBuf>>;
    if image_dir_or_file.is_file() {
        if output_dir_or_file.exists() && !output_dir_or_file.is_file() {
            panic!(
                "\"{}\" already existed but not suitable as output file",
                output_dir_or_file.to_string_lossy()
            )
        }
        srcs = Box::new(vec![Ok(image_dir_or_file)].into_iter());
        dsts = Box::new(vec![output_dir_or_file].into_iter());
    } else if image_dir_or_file.is_dir() {
        if output_dir_or_file.exists() && !output_dir_or_file.is_dir() {
            panic!(
                "\"{}\" already existed but not suitable as output dir",
                output_dir_or_file.to_string_lossy()
            )
        }
        util::create_dir(&output_dir_or_file);
        srcs = Box::new(
            util::whether_dir(image_dir_or_file, "images", "image", verbose)
                .skip(i_skip)
                .step_by(i_step),
        );
        dsts = Box::new(
            (i_ctr..=u32::MAX)
                .into_iter()
                .map(|n| output_dir_or_file.join(format!("{:06}.png", n))),
        );
    } else {
        panic!(
            "Invalid image(s) path \"{}\"",
            image_dir_or_file.to_string_lossy()
        );
    }
    let mut now = Instant::now();
    for (ctr, (src, dst)) in srcs.zip(dsts).enumerate() {
        if verbose {
            print!("[{:06}] ", ctr);
        }
        #[rustfmt::skip]
        let mut img = util::img3(
            match src {
                Ok(p) => {
                    if verbose {
                        print!("\"{}\" ", p.file_name().unwrap().to_string_lossy());
                    }
                    match image::open(&p) {
                        Ok(i) => DynamicImage::ImageLuma8(i.to_luma8()),
                        Err(e) => { match verbose {
                            true => println!("Failed to open: {:?}", e),
                            false => print!("F"),
                        } continue },
                    }
                },
                Err(e) => { match verbose {
                    true => println!("Failed to access: {}", e),
                    false => print!("E"),
                } continue },
            },
            crop,
            resize,
            zoom,
            Lanczos3,
        ).to_luma8();
        img = canny(img, sigma, thr_strong, thr_weak)
            .as_image()
            .to_luma8();
        img.pixels_mut().for_each(|Luma([n])| {
            if *n != 0 {
                *n = 255;
            }
        });
        match img.save(&dst) {
            Ok(_) => match verbose {
                true => {
                    println!("{:05.3} secs", now.elapsed().as_secs_f32());
                    now = Instant::now();
                }
                false => {
                    if ctr % 50 == 0 {
                        print!("[{}]", ctr);
                    } else {
                        print!(".");
                    }
                }
            },
            Err(e) => match verbose {
                true => println!("Failed to save to \"{}\": {:?}", dst.to_string_lossy(), e),
                false => print!("S"),
            },
        }
        stdout().flush().ok();
    }
}
