// Modified from halfblocks.rs
use std::ffi::CString;

use ansi_to_tui::IntoText;
use image::{DynamicImage, Rgb};
use ratatui::{buffer::Buffer, layout::Rect};

use super::{Protocol, StatefulProtocol};
use crate::{ImageSource, Resize, Result};

#[derive(Clone, Default)]
pub struct Chafas {
    data: String,
    rect: Rect,
}

impl Chafas {
    /// Create a FixedHalfblocks from an image.
    ///
    /// The "resolution" is determined by the font size of the terminal. Smaller fonts will result
    /// in more half-blocks for the same image size. To get a size independent of the font size,
    /// the image could be resized in relation to the font size beforehand.
    pub fn from_source(
        source: &ImageSource,
        resize: Resize,
        background_color: Option<Rgb<u8>>,
        area: Rect,
    ) -> Result<Self> {
        let (image, desired) = resize
            .resize(source, Rect::default(), area, background_color, false)
            .unwrap_or_else(|| (source.image.clone(), source.desired));
        let data = encode(&image, desired);
        Ok(Self {
            data,
            rect: desired,
        })
    }
}

fn encode(img: &DynamicImage, rect: Rect) -> String {
    let width = rect.width as u32;
    let height = rect.height as u32;

    let symbol_map = unsafe {
        let symbol_map = chafa_sys::chafa_symbol_map_new();
        chafa_sys::chafa_symbol_map_add_by_tags(
            symbol_map,
            chafa_sys::ChafaSymbolTags_CHAFA_SYMBOL_TAG_ALL,
        );
        symbol_map
    };

    let config = unsafe {
        let config = chafa_sys::chafa_canvas_config_new();
        chafa_sys::chafa_canvas_config_set_geometry(config, width as i32, height as i32);
        chafa_sys::chafa_canvas_config_set_symbol_map(config, symbol_map);
        config
    };

    let canvas = unsafe { chafa_sys::chafa_canvas_new(config) };

    // RGB channels
    let channels = 3;
    let pixels = img.to_rgb8();

    unsafe {
        chafa_sys::chafa_canvas_draw_all_pixels(
            canvas,
            chafa_sys::ChafaPixelType_CHAFA_PIXEL_RGB8,
            pixels.as_ptr(),
            pixels.width() as i32,
            pixels.height() as i32,
            (pixels.width() * channels) as i32,
        );
    }

    let gstring = unsafe { chafa_sys::chafa_canvas_build_ansi(canvas) };
    let ansistr = unsafe { (*gstring).str_ };
    let ansistr = unsafe { CString::from_raw(ansistr) };
    let ansistr = ansistr.to_string_lossy();

    ansistr.to_string()
}

impl Protocol for Chafas {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let text = self.data.into_text().unwrap();
        for (y, line) in text.lines.iter().enumerate() {
            buf.set_line(area.x, area.y + y as u16, line, area.width);
        }
    }

    fn rect(&self) -> Rect {
        self.rect
    }
}

#[derive(Clone)]
pub struct StatefulChafa {
    source: ImageSource,
    current: Chafas,
    hash: u64,
}

impl StatefulChafa {
    pub fn new(source: ImageSource) -> Self {
        StatefulChafa {
            source,
            current: Chafas::default(),
            hash: u64::default(),
        }
    }
}

impl StatefulProtocol for StatefulChafa {
    fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect> {
        resize.needs_resize(&self.source, self.current.rect, area, false)
    }
    fn resize_encode(&mut self, resize: &Resize, background_color: Option<Rgb<u8>>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let force = self.source.hash != self.hash;
        if let Some((img, rect)) = resize.resize(
            &self.source,
            self.current.rect,
            area,
            background_color,
            force,
        ) {
            let data = encode(&img, rect);
            let current = Chafas { data, rect };
            self.current = current;
            self.hash = self.source.hash;
        }
    }
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Chafas::render(&self.current, area, buf);
    }
}
