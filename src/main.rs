extern crate freetype;
extern crate image;

use freetype::Library;
use std::fs::File;
use std::io::Write;
use std::mem;
use std::process;


const FONT_FILE: &str = "assets/FreeMono.ttf";
const PNG_OUTPUT_IMAGE: &str = "atlas.png";
const ATLAS_META_FILE: &str = "atlas.meta";


#[derive(Clone)]
struct GlyphImage {
    data: Vec<u8>,
}

impl GlyphImage {
    fn new(data: Vec<u8>) -> GlyphImage {
        GlyphImage {
            data: data,
        }
    }
}

#[derive(Clone)]
enum GlyphSlot {
    Occupied(GlyphImage),
    Unoccupied,
}

fn create_glyph_image(glyph: &freetype::glyph_slot::GlyphSlot) -> GlyphImage {
    let bitmap = glyph.bitmap();
    let rows = bitmap.rows() as usize;
    let pitch = bitmap.pitch() as usize;
    let mut glyph_data = vec![0 as u8; rows * pitch];
    glyph_data.clone_from_slice(bitmap.buffer());

    GlyphImage::new(glyph_data)
}

fn main() {
    // Init the library
    let ft = match Library::init() {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Failed to initialize FreeType library.");
            panic!(); // process::exit(1);
        }
    };
    // Load a font face
    let face = match ft.new_face(FONT_FILE, 0) {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Could not open font file.");
            panic!(); // process::exit(1);
        }
    };

    let atlas_dimensions_px = 2048;        // atlas size in pixels
    let atlas_columns = 16;                // number of glyphs across atlas
    let padding_px = 6;                    // total space in glyph size for outlines
    let slot_glyph_size = 128;             // glyph maximum size in pixels
    let atlas_glyph_px = 128 - padding_px; // leave some padding for outlines

    // Next we can open a file stream to write our atlas image to
    let mut atlas_buffer = vec![
        0 as u8; atlas_dimensions_px * atlas_dimensions_px * 4 * mem::size_of::<u8>()
    ];
    let mut atlas_buffer_index = 0;

    // I'll tell FreeType the maximum size of each glyph in pixels
    let mut grows = vec![0 as i32; 256];                     // glyph height in pixels
    let mut gwidth = vec![0 as i32; 256];                    // glyph width in pixels
    let mut gpitch = vec![0 as i32; 256];                    // bytes per row of pixels
    let mut gymin = vec![0 as i64; 256];                     // offset for letters that dip below baseline like g and y
    let mut glyph_buffer = vec![GlyphSlot::Unoccupied; 256]; // stored glyph images
    
    // set height in pixels width 0 height 48 (48x48)
    match face.set_pixel_sizes(0, atlas_glyph_px) {
        Ok(_) => {}
        Err(_) => {
            eprintln!("Could not set pixel size for font");
            panic!(); // process::exit(1);
        }
    };

    for i in 33..256 {
        if face.load_char(i, freetype::face::LoadFlag::RENDER).is_err() {
            eprintln!("Could not load character {:x}", i);
            panic!(); // process::exit(1);
        }

        // draw glyph image anti-aliased
        let glyph_handle = face.glyph();
        if glyph_handle.render_glyph(freetype::render_mode::RenderMode::Normal).is_err() {
            eprintln!("Could not render character {:x}", i);
            panic!(); // process::exit(1);
        }

        // get dimensions of bitmap
        grows[i] = glyph_handle.bitmap().rows();
        gwidth[i] = glyph_handle.bitmap().width();
        gpitch[i] = glyph_handle.bitmap().pitch();

        // copy glyph data into memory because it seems to be overwritten/lost later
        glyph_buffer[i] = GlyphSlot::Occupied(create_glyph_image(glyph_handle));

        // get y-offset to place glyphs on baseline. this is in the bounding box
        let glyph = match glyph_handle.get_glyph() {
            Ok(val) => val,
            Err(_) => {
                eprintln!("Could not get glyph handle {}", i);
                panic!(); //process::exit(1);
            }
        };


        // get bbox. "truncated" mode means get dimensions in pixels
        let bbox = glyph.get_cbox(freetype::ffi::FT_GLYPH_BBOX_TRUNCATE);
        gymin[i] = bbox.yMin;
    }

    for y in 0..atlas_dimensions_px {
        for x in 0..atlas_dimensions_px {
            // work out which grid slot[col][row] we are in e.g out of 16x16
            let col = x / slot_glyph_size;
            let row = y / slot_glyph_size;
            let order = row * atlas_columns + col;
            let glyph_index = order + 32;

            // an actual glyph bitmap exists for these indices
            if (glyph_index > 32) && (glyph_index < 256) {
                // pixel indices within padded glyph slot area
                let x_loc = ((x % slot_glyph_size) as i32) - ((padding_px / 2) as i32);
                let y_loc = ((y % slot_glyph_size) as i32) - ((padding_px / 2) as i32);
                // outside of glyph dimensions use a transparent, black pixel (0,0,0,0)
                if x_loc < 0 || y_loc < 0 || x_loc >= gwidth[glyph_index] ||
                         y_loc >= grows[glyph_index] {
                    atlas_buffer[atlas_buffer_index] = 0;
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = 0;
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = 0;
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = 0;
                    atlas_buffer_index += 1;
                } else {
                    // this is 1, but it's safer to put it in anyway
                    // int bytes_per_pixel = gwidth[glyph_index] / gpitch[glyph_index];
                    // int bytes_in_glyph = grows[glyph_index] * gpitch[glyph_index];
                    let byte_order_in_glyph = y_loc * gwidth[glyph_index] + x_loc;
                    let mut colour = [0 as u8; 4];
                    colour[0] = match &glyph_buffer[glyph_index] {
                        GlyphSlot::Occupied(glyph_image) => {
                            glyph_image.data[byte_order_in_glyph as usize]
                        }
                        GlyphSlot::Unoccupied => {
                            panic!("Something went wrong!");
                        }
                    };
                    colour[1] = colour[0];
                    colour[2] = colour[0];
                    colour[3] = colour[0];
                    // print byte from glyph
                    atlas_buffer[atlas_buffer_index] = match &glyph_buffer[glyph_index] {
                        GlyphSlot::Occupied(glyph_image) => {
                            glyph_image.data[byte_order_in_glyph as usize]
                        }
                        GlyphSlot::Unoccupied => {
                            panic!("Something went wrong!");
                        }
                    };
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = match &glyph_buffer[glyph_index] {
                        GlyphSlot::Occupied(glyph_image) => {
                            glyph_image.data[byte_order_in_glyph as usize]
                        }
                        GlyphSlot::Unoccupied => {
                            panic!("Something went wrong!");
                        }
                    };
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = match &glyph_buffer[glyph_index] {
                        GlyphSlot::Occupied(glyph_image) => {
                            glyph_image.data[byte_order_in_glyph as usize]
                        }
                        GlyphSlot::Unoccupied => {
                            panic!("Something went wrong!");
                        }
                    };
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = match &glyph_buffer[glyph_index] {
                        GlyphSlot::Occupied(glyph_image) => {
                            glyph_image.data[byte_order_in_glyph as usize]
                        }
                        GlyphSlot::Unoccupied => {
                            panic!("Something went wrong!");
                        }
                    };
                    atlas_buffer_index += 1;
                }
                // write black in non-graphical ASCII boxes
            } else {
                atlas_buffer[atlas_buffer_index] = 0;
                atlas_buffer_index += 1;
                atlas_buffer[atlas_buffer_index] = 0;
                atlas_buffer_index += 1;
                atlas_buffer[atlas_buffer_index] = 0;
                atlas_buffer_index += 1;
                atlas_buffer[atlas_buffer_index] = 0;
                atlas_buffer_index += 1;
            }
        }
    }

    // write meta-data file to go with atlas image
    let mut file = match File::create(ATLAS_META_FILE) {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Failed to create atlas metadata file {}", ATLAS_META_FILE);
            panic!(); // process::exit(1);
        }
    };
    // comment, reminding me what each column is
    writeln!(file, "// ascii_code prop_xMin prop_width prop_yMin prop_height prop_y_offset").unwrap();
    // write an unique line for the 'space' character
    writeln!(file, "32 0 {} 0 {} 0\n", 0.5 as f32, 1.0 as f32).unwrap();
    // write a line for each regular character
    for i in 33..256 {
        let order = i - 32;
        let col = order % atlas_columns;
        let row = order % atlas_columns;
        let x_min = (col * slot_glyph_size) as f32 / atlas_dimensions_px as f32;
        let y_min = (row * slot_glyph_size) as f32 / atlas_dimensions_px as f32;
        writeln!(file, "{} {} {} {} {} {}", i, x_min, 
            (gwidth[i] + padding_px as i32) as f32 / slot_glyph_size as f32, y_min,
            (grows[i] + padding_px as i32) as f32 / slot_glyph_size as f32,
            -(padding_px as f32 - gymin[i] as f32) / slot_glyph_size as f32
        ).unwrap();
    }
    
    // use stb_image_write to write directly to png
    if image::save_buffer(
        PNG_OUTPUT_IMAGE, &atlas_buffer, 
        atlas_dimensions_px as u32, atlas_dimensions_px as u32, image::RGBA(8)).is_err() {

        eprintln!("ERROR: Could not write file {}", PNG_OUTPUT_IMAGE);
        panic!(); // process::exit(1);
    }
}

