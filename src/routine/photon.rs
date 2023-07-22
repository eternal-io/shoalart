use crate::*;

/// Create Photomosaic for images from Imageset
#[derive(StructOpt, Debug)]
pub struct Param {
    #[structopt(parse(from_os_str))]
    image_dir_or_file: PathBuf,
    #[structopt(parse(from_os_str))]
    output_dir_or_file: PathBuf,

    /// Imageset to be used
    #[structopt(parse(from_os_str))]
    imageset: PathBuf,
    /// Original images which used to generate Imageset
    ///
    /// NOTICE: Filename changes are not allowed.
    #[structopt(parse(from_os_str))]
    imageset_dir: PathBuf,

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
    /// Enlarge each block after process
    ///
    /// Syntax: `{nwidth}x{nheight}` (unit: px; Positive numbers only)
    #[structopt(long, default_value = "40x30", parse(try_from_str = opt_resize))]
    enlarge: (u32, u32),

    /// Invert dark and light; Not recommended for use
    #[structopt(long)]
    negate: bool,
}

pub fn main(
    Param {
        image_dir_or_file,
        output_dir_or_file,
        imageset,
        imageset_dir,
        crop,
        resize,
        enlarge,
        negate,
    }: Param,
) {
}
