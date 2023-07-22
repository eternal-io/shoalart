use crate::*;

/// Custom your own imageset
#[derive(StructOpt, Debug)]
pub struct Param {
    #[structopt(parse(from_os_str))]
    image_dir: PathBuf,
    #[structopt(default_value = "Shoalart-Imageset.bin", parse(from_os_str))]
    output_file: PathBuf,

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

    /// Use `Gray` instead of `RGB` mode
    #[structopt(short, long)]
    gray: bool,
    /// (For debugging)
    #[structopt(long)]
    dump: bool,
}

pub fn main(
    Param {
        image_dir,
        output_file,
        crop,
        resize,
        gray,
        dump,
    }: Param,
) {
}
