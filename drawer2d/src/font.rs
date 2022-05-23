use swash::{Attributes, CacheKey, Charmap, FontRef};

pub struct Font {
    // Full content of the font file
    data: Vec<u8>,
    // Offset to the table directory
    offset: u32,
    // Cache key
    key: CacheKey,
}

pub struct Face<'a> {
    pub font_ref: FontRef<'a>,
    pub size: u32, // size is divided by 100, 100 = 1.0, 2575 = 25.75
}

impl Font {
    pub fn from_file(path: &str, index: usize) -> Option<Self> {
        // Read the full font file
        let data = std::fs::read(path).ok()?;
        // Create a temporary font reference for the first font in the file.
        // This will do some basic validation, compute the necessary offset
        // and generate a fresh cache key for us.
        let font = FontRef::from_index(&data, index)?;
        let (offset, key) = (font.offset, font.key);
        // Return our struct with the original file data and copies of the
        // offset and key from the font reference
        Some(Self { data, offset, key })
    }

    // As a convenience, you may want to forward some methods.
    pub fn attributes(&self) -> Attributes {
        self.as_ref().attributes()
    }

    pub fn charmap(&self) -> Charmap {
        self.as_ref().charmap()
    }

    // Create the transient font reference for accessing this crate's
    // functionality.
    pub fn as_ref(&self) -> FontRef {
        // Note that you'll want to initialize the struct directly here as
        // using any of the FontRef constructors will generate a new key which,
        // while completely safe, will nullify the performance optimizations of
        // the caching mechanisms used in this crate.
        FontRef {
            data: &self.data,
            offset: self.offset,
            key: self.key,
        }
    }
}

impl<'a> Face<'a> {
    pub fn from_font(font: &'a Font, size: f32) -> Self {
        Self {
            font_ref: font.as_ref(),
            size: (size * 100.0) as u32,
        }
    }

    pub fn get_size(&self) -> f32 {
        (self.size as f32) / 100.0
    }
}
