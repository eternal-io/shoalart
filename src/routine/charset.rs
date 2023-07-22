use crate::*;
use image::{
    imageops::{self, Nearest, Triangle},
    GrayImage, Luma,
};
use rusttype::{point, Font, Scale};
use std::{
    fs::{self, File},
    io::{self, stdout, Read, Write},
};
use unicode_width::UnicodeWidthChar;

/// Routines about charset
#[derive(StructOpt, Debug)]
pub enum Param {
    Gen(ParamGen),
    Merge(ParamMerge),
    Read(ParamRead),
}

/// Custom your own charset
#[derive(StructOpt, Debug)]
pub struct ParamGen {
    chars: String,
    #[structopt(parse(from_os_str))]
    font_file: PathBuf,
    #[structopt(default_value = "Shoalart-Charset.bin", parse(from_os_str))]
    output_file: PathBuf,

    /// Use `Compatibility` with optional specified offsets instead of `Adaptive` mode
    #[structopt(short = "C", long = "compat")]
    compat_mode: bool,
    /// Specify offsets for Compatibility mode
    ///
    /// SYNTAX: {width}x{height}+{left}+{top} (unit: px; Negatives are available for offsets)
    #[structopt(short = "A", long = "off", default_value = "64x64+0+0", parse(try_from_str = opt_crop))]
    compat_area: (i32, i32, i32, i32),

    /// (For debugging)
    #[structopt(long)]
    dump: bool,
}

/// Merge charsets
#[derive(StructOpt, Debug)]
pub struct ParamMerge {
    #[structopt(parse(from_os_str))]
    output_file: PathBuf,
    #[structopt(required = true, parse(from_os_str))]
    charset_files: Vec<PathBuf>,
}

/// Open a charset
#[derive(StructOpt, Debug)]
pub struct ParamRead {
    #[structopt(parse(from_os_str))]
    charset_file: PathBuf,
}

const CST_HEADER: &str = "Shoalart.v0 CHR";
const CST_HEADER_LEN: usize = CST_HEADER.len();
/// `width/bool`; `glyph/char`; `feature/f32*10`
const CST_ITEM_LEN: usize = 1 + 4 + 10 * 4;

const CANVAS_SIZE: u32 = 96;
const FONT_SCALE: Scale = Scale { x: 64., y: 64. };
const GLYPH_OFFSET: f32 = 16.;

const BLACK: Luma<u8> = Luma([0]);

////////////////////////////////////////

pub fn read_charset<P: AsRef<Path>>(p: P) -> Result<AHashMap<char, (bool, [f32; 10])>, String> {
    let mut file = match File::open(p.as_ref()) {
        Ok(f) => f,
        Err(e) => Err(format!("Failed to open charset: {:?}", e))?,
    };
    let mut buf: [u8; CST_ITEM_LEN] = unsafe_init!();
    if let Err(e) = file.read_exact(&mut buf[..CST_HEADER_LEN]) {
        Err(format!("Failed to read charset: {:?}", e))?;
    }
    if &buf[..CST_HEADER_LEN] != CST_HEADER.as_bytes() {
        Err(format!("Failed to parse charset: Invalid header"))?;
    }
    let mut comp = util::lz4read(file);
    return match || -> io::Result<AHashMap<char, (bool, [f32; 10])>> {
        let mut cs = AHashMap::with_capacity(384);
        let mut n = comp.read(&mut buf)?;
        while n == CST_ITEM_LEN {
            let c = match char::from_u32(u32::from_be_bytes(buf[0..4].try_into().unwrap())) {
                Some(c) => c,
                None => continue,
            };
            let w = buf[4] != 0;
            cs.insert(
                c,
                (
                    w,
                    (5..CST_ITEM_LEN)
                        .step_by(4)
                        .map(|i| f32::from_be_bytes(buf[i..i + 4].try_into().unwrap()))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                ),
            );
            n = comp.read(&mut buf)?;
        }
        Ok(cs)
    }() {
        Ok(cs) => Ok(cs),
        Err(e) => Err(format!("Failed to parse charset: {:?}", e)),
    };
}

fn write_charset<'a, I, P: AsRef<Path>>(p: P, cs: I) -> io::Result<()>
where
    I: Iterator<Item = (&'a char, &'a bool, &'a [f32; 10])>,
{
    let mut file = File::create(p.as_ref())?;
    file.write(CST_HEADER.as_bytes())?;
    let mut comp = util::lz4write(file);
    comp.write_all(b"\x00\x00\x00\x20\x00")?;
    // 别特么忘了我们的值域是`[-1, 1)`！
    comp.write_all(&(-32f32).to_be_bytes())?;
    (1..10).try_for_each(|_| comp.write_all(&0f32.to_be_bytes()))?;
    for (c, w, feat) in cs {
        comp.write_all(&(*c as u32).to_be_bytes())?;
        comp.write_all(&(*w as u8).to_be_bytes())?;
        feat.iter()
            .try_for_each(|f| comp.write_all(&f.to_be_bytes()))?;
    }
    comp.finish()?;
    return Ok(());
}

////////////////////////////////////////

pub fn main(param: Param) {
    match param {
        Param::Gen(param) => main_gen(param),
        Param::Merge(param) => main_merge(param),
        Param::Read(param) => main_read(param),
    }
}

fn main_gen(
    ParamGen {
        chars,
        font_file,
        output_file,
        compat_mode,
        compat_area,
        dump,
    }: ParamGen,
) {
    let font = util::purify_opt(
        &format!("Failed to open font \"{}\"", font_file.to_string_lossy()),
        Font::try_from_vec(util::purify_err(
            &format!("Failed to access font \"{}\"", font_file.to_string_lossy()),
            fs::read(&font_file),
        )),
    );
    let dump = util::whether_dump(dump, "ShoalartDump-Charset");
    let ascent = {
        let v = font.v_metrics(FONT_SCALE);
        v.ascent + v.line_gap
    };
    let mut block: [[f32; 8]; 8] = unsafe_init!();
    let set_cs = AHashSet::<_>::from_iter(chars.chars());
    let mut cs = Vec::<(char, bool, [f32; 10])>::with_capacity(set_cs.len());
    for (ctr, c) in set_cs.into_iter().enumerate() {
        if ctr % 20 == 0 {
            stdout().flush().ok();
        }
        #[rustfmt::skip]
        let w = match c.width() {
            Some(w) => w - 1 != 0, // false for half & true for full
            None => { print!("K"); continue } // Skipped
        };
        let glyph = font
            .layout(
                &c.to_string(),
                FONT_SCALE,
                point(GLYPH_OFFSET, GLYPH_OFFSET + ascent),
            )
            .next()
            .unwrap();
        #[rustfmt::skip]
        let bound = match glyph.pixel_bounding_box() {
            Some(b) => b,
            None => { print!("K"); continue } // Skipped
        };
        let mut canvas = GrayImage::new(CANVAS_SIZE, CANVAS_SIZE);
        let mut paint = |left, top| {
            glyph.draw(|x, y, a| {
                let x = x as i32 + bound.min.x + left;
                let y = y as i32 + bound.min.y + top;
                if (x >= 0 && x < CANVAS_SIZE as i32) && (y >= 0 && y < CANVAS_SIZE as i32) {
                    canvas.put_pixel(x as u32, y as u32, Luma([(255. * a) as u8]));
                }
            })
        };
        #[rustfmt::skip]
        let img = if compat_mode {
            let (width, height, left, top) = compat_area;
            paint(left, top);
            let real = imageops::crop_imm(
                &canvas,
                GLYPH_OFFSET as u32,
                GLYPH_OFFSET as u32,
                width as u32,
                height as u32,
            ).to_image();
            imageops::resize(&real, 8, 8, Triangle)
        } else {
            paint(0, 0);
            let mut sx = 0;
            let mut ex = CANVAS_SIZE - 1;
            let mut sy = 0;
            let mut ey = CANVAS_SIZE - 1;
            let mut succ = false;
            for x in  0..CANVAS_SIZE        { for y in 0..CANVAS_SIZE {
                if *canvas.get_pixel(x, y) == BLACK { continue }
                else { sx += x; succ = true; break }
            } if succ { break } } succ = false;
            for x in (0..CANVAS_SIZE).rev() { for y in 0..CANVAS_SIZE {
                if *canvas.get_pixel(x, y) == BLACK { continue }
                else { ex -= x; succ = true; break }
            } if succ { break } } succ = false;
            for y in  0..CANVAS_SIZE        { for x in 0..CANVAS_SIZE {
                if *canvas.get_pixel(x, y) == BLACK { continue }
                else { sy += y; succ = true; break }
            } if succ { break } } succ = false;
            for y in (0..CANVAS_SIZE).rev() { for x in 0..CANVAS_SIZE {
                if *canvas.get_pixel(x, y) == BLACK { continue }
                else { ey -= y; succ = true; break }
            } if succ { break } }
            let lx = CANVAS_SIZE - sx - ex;
            let ly = CANVAS_SIZE - sy - ey;
            let real = imageops::crop_imm(&canvas, sx, sy, lx, ly);
            let mut lm = if lx > ly {
                sy = (lx - ly) >> 1;
                sx = 0;
                lx
            } else {
                sx = (ly - lx) >> 1;
                sy = 0;
                ly
            };
            if !w {
                lm <<= 1;
                sy = (lm - ly) >> 1;
            }
            let mut canvas = GrayImage::new(lm, lm);
            imageops::replace(&mut canvas, &real, sx, sy);
            imageops::resize(&canvas, 8, 8, Triangle)
        };
        unsafe {
            img.pixels().enumerate().for_each(|(i, Luma([n]))| {
                *block.as_mut_ptr().cast::<f32>().add(i) = *n as f32 / 128. - 1.
            });
        }
        let feat = if !w {
            algorithm::dct_4x8_feature(&block)
        } else {
            algorithm::dct_8x8_feature(&block)
        };
        cs.push((c, w, feat));
        if let Some(p) = &dump {
            canvas
                .save(p.join(format!("_U{:04X}.png", u32::from(c))))
                .ok();
            if !w {
                imageops::resize(&imageops::crop_imm(&img, 0, 0, 4, 8), 24, 48, Nearest)
            } else {
                imageops::resize(&img, 48, 48, Nearest)
            }
            .save(p.join(format!("U{:04X}.png", u32::from(c))))
            .ok();
        }
        print!(".") // OK!
    }
    println!("\nTotally {} chars.", cs.len() + 1);
    try_again!(
        write_charset(&output_file, cs.iter().map(|(c, w, f)| (c, w, f))),
        "Failed to write charset \"{}\": {:?}",
        output_file.to_string_lossy(),
    );
}

fn main_merge(
    ParamMerge {
        output_file,
        charset_files,
    }: ParamMerge,
) {
    let mut cs = AHashMap::<char, (bool, [f32; 10])>::with_capacity(2048);
    for p in charset_files {
        print!("File \"{}\": ", p.to_string_lossy());
        match read_charset(&p) {
            Ok(c) => cs.extend(c),
            Err(e) => {
                println!("{}", e);
                continue;
            }
        };
        println!("Ok")
    }
    if cs.is_empty() {
        panic!("No inputs")
    }
    println!("Totally {} chars.", cs.len());
    try_again!(
        write_charset(&output_file, cs.iter().map(|(c, (w, f))| (c, w, f))),
        "Failed to write charset \"{}\": {:?}",
        output_file.to_string_lossy(),
    );
}

#[rustfmt::skip]
fn main_read(ParamRead { charset_file }: ParamRead) {
    let mut cs = read_charset(&charset_file)
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();
    cs.sort_unstable_by_key(|v| v.0);
    cs.iter().for_each(|(c, (w, f))| println!(
        "{} / ('{}', [{:>10.06},{:>10.06},{:>10.06},{:>10.06},{:>10.06},{:>10.06},{:>10.06},{:>10.06},{:>10.06},{:>10.06}]),",
        *w as u8, c,
        f[0], f[1], f[2], f[3], f[4],
        f[5], f[6], f[7], f[8], f[9],
    ));
    println!("Totally {} chars.", cs.len());
}
