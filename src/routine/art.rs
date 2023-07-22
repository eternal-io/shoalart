use crate::*;
use crossterm::{
    cursor::{Hide as HideCursor, MoveTo, MoveToNextLine, Show as ShowCursor},
    queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use image::{
    imageops::{self, Lanczos3, Triangle},
    GrayImage, Luma, Rgb, RgbImage,
};
use scrap;
use std::{
    fs::File,
    io::{self, stdout, Read, Write},
    time::{Duration, Instant},
};

/// Routines about ASCII art
#[derive(StructOpt, Debug)]
pub enum Param {
    Make(ParamMake),
    Play(ParamPlay),
}

/// Create ASCII Art for images from Charset
///
/// Use a unique format for storage, which suffixed with `.shoal` and included colors.
#[derive(StructOpt, Debug)]
pub struct ParamMake {
    #[structopt(parse(from_os_str))]
    image_dir_or_file: PathBuf,
    #[structopt(parse(from_os_str))]
    output_dir_or_file: PathBuf,
    /// Linking color
    ///
    /// NOTICE: If the two are not the same size, then the colorize image
    /// will be resized to the same size as the original image.
    ///
    /// If this field was skipped, or provided images are invalid,
    /// the color of original image will be used.
    ///
    /// Colorize image will be also `crop` then `resize`.
    #[structopt(long = "color", default_value = "", parse(from_os_str))]
    colorize_dir_or_file: PathBuf,

    /// Charset to be used; Bulit-in `chars/ASCII+font/Sarasa-Term-SC` by default
    #[structopt(short, long, parse(from_os_str))]
    charset: Option<PathBuf>,

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

    /// Invert dark and light; Not recommended for use
    #[structopt(short, long)]
    negate: bool,

    /// Specify the value of skipping first N COLOR files
    #[structopt(long = "skip", default_value = "0")]
    i_skip: usize,
    /// Sepcify the step of skipping COLOR files
    #[structopt(long = "step", default_value = "1")]
    i_step: usize,
    /// Specify the start value of OUTPUT filename
    #[structopt(long = "ctr", default_value = "1")]
    i_ctr: u32,

    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,
}

/// Play ASCII animation on your terminal
#[derive(StructOpt, Debug)]
pub struct ParamPlay {
    #[structopt(parse(from_os_str))]
    shoal_dir_or_file: PathBuf,

    /// Set the left mergin of animation
    #[structopt(short = "x", default_value = "0")]
    sx: u16,
    /// Set the top mergin of animation
    #[structopt(short = "y", default_value = "0")]
    sy: u16,

    /// Maximum frame rate during play
    ///
    /// On Windows: A too large value (about 5) may prevent the art from being fully captured!
    #[structopt(short = "f", long = "fps", default_value = "5")]
    max_fps: f32,
    /// Enable capture function; Take screenshot for each frame then save it
    #[structopt(short, long, parse(from_os_str))]
    capture: Option<PathBuf>,

    /// Use no color on your terminal
    #[structopt(short, long = "monoch")]
    monoch: bool,

    /// Specify the start value of OUTPUT filename
    #[structopt(long = "ctr", default_value = "1")]
    i_ctr: u32,
}

////////////////////////////////////////

const ART_HEADER: &str = "Shoalart.v0 ART";
const ART_HEADER_LEN: usize = ART_HEADER.len();

pub fn read_art<P: AsRef<Path>>(p: P) -> Result<Vec<Vec<([u8; 3], char)>>, String> {
    let mut file = match File::open(p.as_ref()) {
        Ok(f) => f,
        Err(e) => Err(format!("Failed to open art: {:?}", e))?,
    };
    let mut buf: [u8; ART_HEADER_LEN] = unsafe_init!();
    if let Err(e) = file.read_exact(&mut buf) {
        Err(format!("Failed to read art: {:?}", e))?;
    }
    if &buf != ART_HEADER.as_bytes() {
        Err(format!("Failed to parsing art: Invalid header"))?;
    }
    return match || -> io::Result<Vec<Vec<([u8; 3], char)>>> {
        let mut comp = util::lz4read(file);
        comp.read_exact(&mut buf[..2])?;
        let h = u16::from_be_bytes(buf[..2].try_into().unwrap()) as usize;
        let mut lines = Vec::<Vec<([u8; 3], char)>>::with_capacity(h);
        for _ in 0..h {
            comp.read_exact(&mut buf[..2])?;
            let w = u16::from_be_bytes(buf[..2].try_into().unwrap()) as usize;
            let mut line = Vec::<([u8; 3], char)>::with_capacity(w);
            for _ in 0..w {
                comp.read_exact(&mut buf[..7])?;
                let rgb: [u8; 3] = (&buf[..3]).try_into().unwrap();
                let c = unsafe {
                    char::from_u32_unchecked(u32::from_be_bytes(buf[3..7].try_into().unwrap()))
                };
                line.push((rgb, c));
            }
            lines.push(line);
        }
        Ok(lines)
    }() {
        Ok(a) => Ok(a),
        Err(e) => Err(format!("Failed to parsing art: {:?}", e)),
    };
}

pub fn play_art<W: Write>(
    out: &mut W,
    dat: &Vec<Vec<([u8; 3], char)>>,
    sx: u16,
    sy: u16,
    monoch: bool,
) -> io::Result<()> {
    // queue!(out, Clear(ClearType::All))?;
    let mut cc = [0u8, 0, 0];
    for (y, line) in dat.iter().enumerate() {
        queue!(out, MoveTo(sx, sy + y as u16))?;
        for (c, w) in line {
            if !monoch && *c != cc {
                cc = c.clone();
                let [r, g, b] = *c;
                queue!(out, SetForegroundColor(Color::Rgb { r, g, b }))?;
            }
            queue!(out, Print(w))?;
        }
    }
    return Ok(());
}

fn make_art<P: AsRef<Path>>(
    draft: GrayImage,
    color: RgbImage,
    csh: &Vec<(char, [f32; 10])>,
    csf: &Vec<(char, [f32; 10])>,
    p: P,
) -> io::Result<()> {
    let mut file = File::create(p.as_ref())?;
    file.write_all(ART_HEADER.as_bytes())?;
    let w = draft.width();
    let h = draft.height();
    let mut comp = util::lz4write(file);
    comp.write_all(&((h >> 3) as u16).to_be_bytes())?; // lines
    let mut block: [[f32; 8]; 8] = unsafe_init!();
    for y in (0..h).step_by(8) {
        let mut x = 0;
        let mut cache = Vec::<([u8; 3], char)>::with_capacity(w as usize >> 2);
        while x < w - 4 {
            let mut rank = Vec::<(char, bool, f32)>::with_capacity(csh.len() + csf.len());
            let mut im = GrayImage::new(8, 8);
            let wider = x < w - 8;
            imageops::replace(
                &mut im,
                &imageops::crop_imm(&draft, x, y, if wider { 8 } else { 4 }, 8),
                0,
                0,
            );
            unsafe {
                im.pixels().enumerate().for_each(|(i, Luma([n]))| {
                    *block.as_mut_ptr().cast::<f32>().add(i) = *n as f32 / 128. - 1.
                });
            }
            if wider {
                let f = algorithm::dct_8x8_feature(&block);
                csf.iter()
                    .for_each(|(c, f2)| rank.push((*c, true, algorithm::similarity(&f, &f2))));
            }
            let f = algorithm::dct_4x8_feature(&block);
            csh.iter()
                .for_each(|(c, f2)| rank.push((*c, false, algorithm::similarity(&f, &f2))));
            let &(c, w, _) = rank
                .iter()
                .min_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap())
                .unwrap();
            let Rgb(rgb) = *imageops::resize(
                &imageops::crop_imm(&color, x, y, if wider { 8 } else { 4 }, 8).to_image(),
                1,
                1,
                Triangle,
            )
            .get_pixel(0, 0);
            cache.push((rgb, c));
            x += if w { 8 } else { 4 };
        }
        comp.write_all(&(cache.len() as u16).to_be_bytes())?; // each line
        for (rgb, c) in cache {
            comp.write_all(&rgb)?;
            comp.write_all(&(c as u32).to_be_bytes())?;
        }
    }
    comp.finish()?;
    return Ok(());
}

////////////////////////////////////////

pub fn main(param: Param) {
    match param {
        Param::Make(param) => main_make(param),
        Param::Play(param) => main_play(param),
    }
}

fn main_make(
    ParamMake {
        image_dir_or_file,
        output_dir_or_file,
        colorize_dir_or_file,
        charset,
        crop,
        resize,
        zoom,
        negate,
        i_skip,
        i_step,
        i_ctr,
        verbose,
    }: ParamMake,
) {
    let mut csh = Vec::<(char, [f32; 10])>::with_capacity(0);
    let mut csf = Vec::<(char, [f32; 10])>::with_capacity(0);
    if let Some(p) = &charset {
        println!("Use outer charset \"{}\".", p.to_string_lossy());
        let cs = routine::charset::read_charset(p).unwrap();
        csh.reserve_exact(cs.len());
        csf.reserve_exact(cs.len());
        for (c, (w, f)) in cs.into_iter() {
            match w {
                false => csh.push((c, f)),
                true => csf.push((c, f)),
            }
        }
    } else {
        println!("Use built-in charset.");
        csh.reserve_exact(BULITIN_CHARSET.len());
        csh.extend_from_slice(&BULITIN_CHARSET);
    }
    let verbose = verbose > 0;
    let srcs: Box<dyn Iterator<Item = Result<PathBuf, String>>>;
    let dsts: Box<dyn Iterator<Item = PathBuf>>;
    let clrs: Box<dyn Iterator<Item = Result<PathBuf, String>>>;
    if image_dir_or_file.is_file() {
        if output_dir_or_file.exists() && !output_dir_or_file.is_file() {
            panic!(
                "\"{}\" already existed but not suitable as output file",
                output_dir_or_file.to_string_lossy()
            )
        }
        srcs = Box::new(vec![Ok(image_dir_or_file)].into_iter());
        dsts = Box::new(vec![output_dir_or_file].into_iter());
        clrs = Box::new(
            vec![if colorize_dir_or_file.exists() {
                Ok(colorize_dir_or_file)
            } else {
                Err(String::with_capacity(0))
            }]
            .into_iter(),
        );
    } else if image_dir_or_file.is_dir() {
        if output_dir_or_file.exists() && !output_dir_or_file.is_dir() {
            panic!(
                "\"{}\" already existed but not suitable as output dir",
                output_dir_or_file.to_string_lossy()
            )
        }
        util::create_dir(&output_dir_or_file);
        srcs = util::whether_dir(image_dir_or_file, "images", "image", verbose);
        dsts = Box::new(
            (i_ctr..=u32::MAX)
                .into_iter()
                .map(|n| output_dir_or_file.join(format!("{:06}.shoal", n))),
        );
        clrs = if colorize_dir_or_file.exists() {
            Box::new(
                util::whether_dir(colorize_dir_or_file, "color images", "color image", verbose)
                    .chain(std::iter::repeat(Err(String::with_capacity(0))))
                    .skip(i_skip)
                    .step_by(i_step)
                    .into_iter(),
            )
        } else {
            Box::new(std::iter::repeat(Err(String::with_capacity(0))).into_iter())
        };
    } else {
        panic!(
            "Invalid image(s) path \"{}\"",
            image_dir_or_file.to_string_lossy()
        );
    }
    for (ctr, ((src, dst), clr)) in srcs.zip(dsts).zip(clrs).enumerate() {
        if verbose {
            print!("[{:06}] ", ctr);
        }
        #[rustfmt::skip]
        let img = util::img3(
            match src {
                Ok(p) => {
                    if verbose {
                        print!("\"{}\" ", p.file_name().unwrap().to_string_lossy());
                    }
                    match image::open(&p) {
                        Ok(i) => i,
                        Err(e) => { match verbose {
                            true => println!("Failed to open: {:?}", e),
                            false => print!("F"),
                        } continue },
                    }
                },
                Err(e) => { match verbose {
                    true => println!("{}", e),
                    false => print!("E"),
                } continue },
            },
            crop,
            resize,
            zoom,
            Lanczos3,
        );
        let mut draft = img.to_luma8();
        if negate {
            draft.pixels_mut().for_each(|Luma([n])| *n = 255 - *n);
        }
        #[rustfmt::skip]
        let color = match clr {
            Ok(p) => match image::open(&p) {
                Ok(img) => {
                    if verbose { print!("× \"{}\"", p.file_name().unwrap().to_string_lossy()) }
                    util::img3(img, crop, Some(draft.dimensions()), None, Lanczos3)
                },
                Err(e) => { if verbose { print!("(Color unopenable: {:?})", e) } img },
            },
            Err(e) => {
                if verbose { if e.is_empty() {
                        print!("(No color provided)")
                    } else {
                        print!("(Color inaccessible: {})", e)
                    }
                }
                img
            },
        }.to_rgb8();
        match make_art(draft, color, &csh, &csf, dst) {
            Ok(_) => match verbose {
                true => println!(" - Ok"),
                false => {
                    if ctr % 100 == 0 {
                        print!("[{}]", ctr);
                    } else {
                        print!(".");
                    }
                }
            },
            Err(e) => match verbose {
                true => println!(" - Failed to save to: {:?}", e),
                false => print!("S"),
            },
        }
        stdout().flush().ok();
    }
}

fn main_play(
    ParamPlay {
        shoal_dir_or_file,
        sx,
        sy,
        max_fps,
        capture,
        monoch,
        i_ctr,
    }: ParamPlay,
) {
    let srcs: Box<dyn Iterator<Item = Result<PathBuf, String>>>;
    let single: bool;
    if shoal_dir_or_file.is_file() {
        srcs = Box::new(vec![Ok(shoal_dir_or_file)].into_iter());
        single = true;
    } else if shoal_dir_or_file.is_dir() {
        srcs = util::whether_dir(shoal_dir_or_file, "shoals", "shoal", false);
        single = false;
    } else {
        panic!(
            "Invalid shoal(s) path \"{}\"",
            shoal_dir_or_file.to_string_lossy()
        );
    }
    let avg = if max_fps > 0. { 1. / max_fps } else { 0. };
    let mut out = stdout();
    let mut cap = None;
    let mut caps: Box<dyn Iterator<Item = PathBuf>> = Box::new(std::iter::empty());
    if !single {
        if let Some(p) = capture {
            if p.exists() && !p.is_dir() {
                panic!(
                    "\"{}\" already existed but not suitable as capture dir",
                    p.to_string_lossy()
                )
            } else {
                util::create_dir(&p);
                let c = scrap::Capturer::new(scrap::Display::primary().unwrap()).unwrap();
                cap = Some((c.width() as u32, c.height() as u32, c));
                caps = Box::new(
                    (i_ctr..u32::MAX)
                        .into_iter()
                        .map(move |n| p.join(format!("{:06}.png", n))),
                );
            }
        }
        enable_raw_mode().ok();
        queue!(out, EnterAlternateScreen, HideCursor).ok();
    }
    let mut now = Instant::now();
    for src in srcs {
        src.and_then(|p| read_art(&p))
            .and_then(|dat| {
                play_art(&mut out, &dat, sx, sy, monoch).or_else(|e| Err(format!("{:?}", e)))
            })
            .or_else(|e| {
                queue!(
                    out,
                    MoveTo(sx, sy),
                    ResetColor,
                    Print(format!("Invalid frame: {}", e))
                )
            })
            .ok();
        out.flush().ok();
        use crossterm::event::*;
        if poll(Duration::from_millis(1)).unwrap_or(false) {
            if let Some(e) = read().ok() {
                if let Event::Key(k) = e {
                    if (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
                        || k.code == KeyCode::Esc
                    {
                        break;
                    }
                }
            }
        }
        if max_fps > 0. {
            let ext = avg - now.elapsed().as_secs_f32();
            if ext > 0. {
                std::thread::sleep(Duration::from_secs_f32(ext));
            }
            now = Instant::now()
        }
        if let Some((w, h, c)) = &mut cap {
            let (w, h) = (*w, *h);
            for _ in 0..10 {
                match c.frame() {
                    Ok(frame) => {
                        let mut img = RgbImage::new(w, h);
                        unsafe {
                            (0..w * h).for_each(|i| {
                                *img.as_mut_ptr().cast::<[u8; 3]>().add(i as usize) = {
                                    let [b, g, r, _] =
                                        *(*frame).as_ptr().cast::<[u8; 4]>().add(i as usize);
                                    [r, g, b]
                                }
                            })
                        }
                        img.save(caps.next().unwrap()).unwrap();
                    }
                    Err(e) => {
                        if e.kind() == io::ErrorKind::WouldBlock {
                            std::thread::sleep(Duration::from_millis(3));
                            continue;
                        }
                    }
                }
                break;
            }
        }
    }
    if !single {
        queue!(out, LeaveAlternateScreen, ShowCursor, ResetColor).ok();
    } else {
        queue!(out, MoveToNextLine(1), ShowCursor, ResetColor).ok();
    }
    disable_raw_mode().ok();
}

#[rustfmt::skip]
const BULITIN_CHARSET: [(char, [f32; 10]); 95] = [
    (' ', [-32.000000,  0.000000,  0.000000,  0.000000,  0.000000,  0.000000,  0.000000,  0.000000,  0.000000,  0.000000]),
    ('!', [-27.687500,  0.153072,  0.083712, -3.049398,  0.005655, -1.810784, -0.375900, -0.034486, -0.108238, -0.202099]),
    ('"', [-27.671875,  2.566176,  0.122190, -2.032932,  0.075350, -0.864137, -2.685735, -0.016573, -1.201202,  0.092894]),
    ('#', [-18.093750, -0.366140,  0.402446, -5.778388, -0.019004, -6.350002,  0.116994, -0.181468,  0.148199,  0.192067]),
    ('$', [-18.960938,  0.381606,  0.295505, -6.314242,  0.238307, -5.032045,  0.090663, -0.072012, -0.248507, -0.080546]),
    ('%', [-17.656250,  0.521049,  0.519682, -3.425048,  0.848369, -6.567137, -0.180730, -0.137714, -0.070680,  0.215259]),
    ('&', [-19.843750, -0.451876,  0.441400, -5.071281,  0.444430, -5.632273,  1.096957, -0.298243, -0.389327, -0.146957]),
    ('\'',[-29.851562,  1.274990,  0.044846, -1.519175,  0.025806, -0.423975, -1.323533, -0.010575, -0.901554, -0.108267]),
    ('(', [-25.070312, -0.179685,  0.072743, -4.226068, -0.005199, -1.347678, -0.124397, -1.219271,  0.111271, -0.849313]),
    (')', [-25.046875, -0.187074,  0.226566, -4.132155,  0.004431, -1.303859, -0.108054,  1.182301,  0.106552,  0.575849]),
    ('*', [-26.734375,  2.531234,  0.130946, -2.618505,  0.064653, -2.245230, -3.664769, -0.056583, -1.232644, -0.030322]),
    ('+', [-27.835938, -0.120207,  0.092381, -2.292573, -0.001408, -3.551126,  0.276549, -0.078877,  0.065600, -0.080121]),
    (',', [-29.164062, -2.126778,  0.086702, -2.005311, -0.080844,  0.519470,  0.864870,  0.067713,  1.503859, -0.209316]),
    ('-', [-29.851562, -0.065538,  0.056504, -0.867311, -0.001408, -1.984897,  0.186637, -0.052203,  0.026943,  0.006492]),
    ('.', [-30.421875, -1.011232,  0.029897, -1.115903, -0.019085, -0.223716,  1.142172, -0.004576,  0.715049, -0.072178]),
    ('/', [-25.585938, -0.203722,  0.162807, -3.297990, -1.539253, -1.539445, -0.069962,  0.099356, -0.031454, -0.025581]),
    ('0', [-19.375000,  0.446021,  0.460914, -3.159883, -0.047313, -6.233440, -0.185610, -0.262044, -0.115228,  0.275479]),
    ('1', [-26.125000,  0.926521, -0.079960, -3.800699,  0.296921, -2.481578, -0.942908,  0.181746, -0.450515,  0.846323]),
    ('2', [-23.171875,  0.132234,  0.233747, -3.491340, -0.537179, -2.804240, -0.016471,  0.114277, -0.068887, -0.013109]),
    ('3', [-22.796875,  0.200516, -1.060608, -3.513437, -0.036830, -3.416266,  0.128429,  0.920007, -0.247994,  1.192726]),
    ('4', [-24.132812, -0.268562, -0.194082, -4.148728,  0.111756, -4.893866,  1.009882, -0.287851, -0.095038,  1.162668]),
    ('5', [-21.843750,  0.542205,  0.842696, -3.325612,  0.645371, -4.266465, -0.197154, -0.243413, -0.125037, -0.217509]),
    ('6', [-22.726562, -0.949528,  0.758947, -3.574204,  0.094181, -5.027889,  1.275183, -0.584171, -0.096499, -0.607358]),
    ('7', [-25.476562,  1.450719, -0.662002, -3.507912, -0.459557, -2.083646, -0.762941,  0.515639, -0.318104,  0.985762]),
    ('8', [-20.421875,  0.288087,  0.342227, -4.618291, -0.004735, -5.274710,  0.013473, -0.161544, -0.315211,  0.133299]),
    ('9', [-22.765625,  1.573469, -0.187715, -3.646019,  0.169756, -4.681046, -1.681783,  0.286937, -0.125990,  0.759409]),
    (':', [-28.843750, -0.590174,  0.059794, -2.231806, -0.014166, -1.497428,  0.176183, -0.025056,  0.417316, -0.144356]),
    (';', [-27.578125, -1.712216,  0.113609, -3.126738, -0.073439, -0.751253, -0.099595,  0.046090,  1.210719, -0.274277]),
    ('<', [-27.406250, -0.130908, -0.070727, -1.988738, -0.003484, -3.715560,  0.245948, -0.171300,  0.059391,  0.089090]),
    ('=', [-27.687500, -0.121531,  0.127444, -1.745670,  0.000000, -3.739001,  0.214547, -0.109930,  0.049106,  0.018964]),
    ('>', [-27.406250, -0.121995,  0.329843, -1.955592, -0.003926, -3.694419,  0.225264, -0.036171,  0.055572, -0.040955]),
    ('?', [-25.312500,  1.389435, -0.591787, -3.380854, -0.149249, -2.292413, -1.396848,  0.526491, -0.254172,  0.938740]),
    ('@', [-17.453125, -0.174052,  0.558885, -4.872407, -0.205499, -6.259057, -0.182926, -0.194182,  0.166714,  0.569745]),
    ('A', [-22.265625, -0.384896,  0.266846, -4.474660, -0.042674, -5.220045,  0.892567, -0.132697, -0.823877,  0.009057]),
    ('B', [-19.593750,  0.369113,  1.274137, -3.369806,  0.138414, -5.629320, -0.114359, -0.389657, -0.250380,  0.333273]),
    ('C', [-23.421875,  0.266863,  2.007504, -1.999786,  0.067025, -2.803266,  0.148635, -1.471554, -0.057286, -0.436893]),
    ('D', [-20.609375,  0.362096,  0.930460, -2.132369,  0.028989, -4.721627,  0.018683, -0.068627, -0.062999,  0.080986]),
    ('E', [-22.460938,  0.362861,  2.488972, -2.911291,  0.086686, -3.342021, -0.091548, -1.516960, -0.112359, -1.252205]),
    ('F', [-24.476562,  1.371431,  2.149274, -2.988631, -0.179298, -3.216630, -0.914221, -1.225676, -0.686668, -2.187794]),
    ('G', [-21.179688, -0.061635,  0.881086, -2.458301,  0.273389, -4.603578,  0.726455, -0.593764, -0.140191,  0.322677]),
    ('H', [-21.117188,  0.414544,  0.444939, -1.375544,  0.012675, -5.888223, -0.366809, -0.233001, -0.077484,  0.395705]),
    ('I', [-24.187500,  0.239964,  0.180533, -4.474660,  0.010479, -1.988788,  0.202812, -0.040681, -0.145335, -0.170450]),
    ('J', [-24.789062, -0.365011, -1.542677, -2.867097, -0.688413, -2.470557,  0.909462,  0.996825,  0.051710,  2.295300]),
    ('K', [-21.664062,  0.356033,  1.886853, -3.872515,  0.117591, -5.208298, -0.138047, -1.445287, -0.177452, -0.655992]),
    ('L', [-25.593750, -0.923295,  2.244432, -2.220757,  0.346405, -2.359938,  0.814594, -1.297573,  0.481206, -2.376696]),
    ('M', [-18.406250,  1.350361,  0.542274, -1.668330,  0.040686, -7.238306, -1.589606, -0.276051, -0.514539,  0.385285]),
    ('N', [-18.929688,  0.447730,  0.479065, -2.867097,  0.416399, -6.752536, -0.215339, -0.192991, -0.080750,  0.333734]),
    ('O', [-21.007812,  0.379488,  0.421322, -2.104748,  0.013769, -4.736576, -0.031906, -0.222900, -0.066026,  0.309816]),
    ('P', [-22.664062,  1.837856,  1.596265, -2.568786, -0.420267, -4.502777, -1.876660, -0.679979, -0.694728, -1.097693]),
    ('Q', [-19.468750, -0.978253,  0.010881, -2.949961,  0.389398, -3.853688, -0.320380, -0.504509,  0.678237,  0.892407]),
    ('R', [-20.382812,  0.924624,  1.261365, -2.977582,  0.100143, -5.571790, -0.417643, -0.659433, -0.405686,  0.547843]),
    ('S', [-22.718750,  0.274411,  0.224990, -3.513437,  0.329904, -3.469781,  0.059243, -0.018861, -0.147018,  0.110106]),
    ('T', [-25.460938,  1.561839,  0.154652, -3.817272,  0.050030, -1.979819, -0.818250, -0.034012, -0.494736, -0.189627]),
    ('U', [-21.765625, -0.054476,  0.385958, -1.568893,  0.028607, -4.795132,  0.245321, -0.198041,  0.659680,  0.354362]),
    ('V', [-23.179688,  0.715139,  0.262105, -3.817272,  0.052421, -4.416215, -0.553110, -0.114981,  0.747905,  0.040918]),
    ('W', [-19.906250,  0.003179,  0.422861, -3.458194,  0.050549, -6.621926,  0.340759, -0.215284,  1.178285,  0.183611]),
    ('X', [-22.546875,  0.327342,  0.250659, -4.331029,  0.002333, -4.304321, -0.067554, -0.091347, -0.158510,  0.027721]),
    ('Y', [-24.539062,  1.377002,  0.195181, -3.839369,  0.062966, -3.559084, -1.686085, -0.064870, -0.030778, -0.062908]),
    ('Z', [-23.351562,  0.244906,  0.236737, -4.115582, -0.651134, -2.315642,  0.225567, -0.125914, -0.182088, -0.020327]),
    ('[', [-22.742188, -0.214890,  2.780140, -3.806223, -0.075267,  0.151677, -0.322796, -1.158647,  0.085283, -3.118804]),
    ('\\',[-25.593750, -0.103387,  0.170025, -3.303514,  1.522461, -1.556870, -0.099181, -0.126896,  0.212933, -0.022591]),
    (']', [-22.906250, -0.206061, -2.268702, -4.231592,  0.058555,  0.218389, -0.308281,  1.148073,  0.089876,  3.415210]),
    ('^', [-28.789062,  1.792508,  0.091355, -1.508126,  0.049317, -0.982663, -2.421526, -0.030383, -0.932512,  0.004016]),
    ('_', [-29.820312, -1.786733,  0.070940, -0.889408, -0.060062,  0.777963,  0.427701,  0.031054,  0.728649,  0.012472]),
    ('`', [-30.273438,  1.281634,  0.377341, -1.055136,  0.319417,  0.247545, -0.709477,  0.167055, -0.763505, -0.604757]),
    ('a', [-22.882812, -2.044169, -0.272188, -3.054922,  0.009604, -5.289057,  2.390256,  0.304261,  0.504303,  0.310065]),
    ('b', [-21.710938, -0.863091,  1.505796, -2.005311,  0.467237, -5.362862,  0.770163, -0.420108,  0.370968, -0.103511]),
    ('c', [-24.835938, -1.384317,  1.023141, -2.126844, -0.211549, -3.988225,  1.157078, -0.787506,  0.383002, -0.142766]),
    ('d', [-21.843750, -0.897785, -0.736569, -2.209709, -0.517137, -5.339458,  0.811523,  0.006587,  0.275580,  0.675821]),
    ('e', [-22.929688, -1.706877,  0.622571, -2.933388, -0.179431, -5.714520,  2.020030, -0.422397,  0.553496, -0.012721]),
    ('f', [-24.554688,  1.447436,  0.245120, -4.192922, -0.454684, -3.534565, -1.783444, -0.509512, -0.522445, -1.183809]),
    ('g', [-20.007812, -4.601786, -0.017301, -3.297990,  0.028617, -2.984084,  0.471007, -0.078044,  1.550148,  0.347994]),
    ('h', [-22.726562, -0.212798,  1.406198, -1.287155,  0.578000, -5.268217,  0.139215, -0.471167, -0.185641, -0.026379]),
    ('i', [-25.296875, -0.768052,  0.373025, -3.988524,  0.101222, -2.478750,  0.719876, -0.210152,  0.247224, -0.369772]),
    ('j', [-23.765625, -2.434944, -0.225871, -4.607243, -0.643026, -0.477951, -1.323584,  1.036065,  0.780308,  2.341824]),
    ('k', [-22.882812, -0.675447,  2.144093, -3.198553,  0.252043, -5.122835,  1.051329, -1.171199,  0.464536, -0.929968]),
    ('l', [-24.992188, -0.381748,  0.338387, -4.192922,  0.224972, -2.141264,  0.534718,  0.095957,  0.073721, -0.265732]),
    ('m', [-20.914062, -1.815744,  0.523698, -2.292573, -0.030414, -7.030564,  2.053936, -0.340365,  0.172793,  0.123905]),
    ('n', [-24.117188, -1.096497,  0.497728, -1.287155,  0.000696, -5.072948,  1.095973, -0.343601, -0.185641,  0.349921]),
    ('o', [-23.687500, -1.601338,  0.289438, -2.154466, -0.058681, -4.941063,  1.688719, -0.187663,  0.378737,  0.179082]),
    ('p', [-21.664062, -3.041747,  1.546626, -1.994262, -0.962356, -4.267579,  1.348060,  0.296564,  0.367452, -0.120423]),
    ('q', [-21.781250, -2.949914, -0.777399, -2.209709,  0.730617, -4.305420,  1.344067, -0.622644,  0.518247,  0.692734]),
    ('r', [-26.875000, -0.231429,  0.824861, -2.463825, -0.467170, -3.490445, -0.180412, -0.409141,  0.158152, -1.848484]),
    ('s', [-24.343750, -1.529653,  0.195394, -3.303514,  0.097704, -4.396863,  1.499120, -0.220578,  0.625398,  0.038654]),
    ('t', [-24.179688, -0.243258,  0.343143, -3.960903,  0.730226, -3.701440, -0.123235, -0.513858, -0.129830, -1.726683]),
    ('u', [-24.132812, -1.990543,  0.124154, -1.309253,  0.058266, -4.429024,  2.333935, -0.174800,  0.694964,  0.169813]),
    ('v', [-25.507812, -1.060046,  0.182197, -2.845000, -0.006952, -4.296026,  1.360911, -0.133564,  1.038488,  0.050100]),
    ('w', [-23.234375, -1.954892,  0.297169, -2.508019, -0.040788, -5.579309,  2.802272, -0.204906,  1.194064,  0.140004]),
    ('x', [-24.820312, -1.406142,  0.194155, -3.375330, -0.037798, -4.350866,  1.577819, -0.106498,  0.685004,  0.021228]),
    ('y', [-24.085938, -2.333444,  0.411166, -4.038242, -0.221094, -3.441071,  1.018816,  0.042772,  1.898888, -0.523097]),
    ('z', [-24.726562, -1.385258,  0.200135, -3.375330, -0.324105, -3.633438,  0.749318,  0.132319,  0.634341,  0.006793]),
    ('{', [-23.804688, -0.203985,  0.034213, -5.098903, -0.000923, -1.509071, -0.122314, -0.880701,  0.121482, -0.307164]),
    ('|', [-25.820312, -0.151578,  0.128558, -4.369699, -0.002932, -1.530989, -0.072253, -0.032672,  0.107182, -0.310366]),
    ('}', [-23.812500, -0.210658,  0.306226, -5.038136, -0.004737, -1.456795, -0.130692,  0.826880,  0.126706, -0.126843]),
    ('~', [-27.789062, -0.437989,  0.137138, -1.265058, -0.016172, -3.733959,  1.073460, -0.124296,  0.129772,  0.056805]),
];
