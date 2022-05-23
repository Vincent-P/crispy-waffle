use crate::font::*;
use crate::glyph_cache::*;
use crate::rect::Rect;
use std::mem::size_of;
use swash::shape::ShapeContext;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct ColorU32(pub u32);

impl ColorU32 {
    pub fn from_u32(r: u32, g: u32, b: u32, a: u32) -> Self {
        let r = r & 0xFF;
        let g = g & 0xFF;
        let b = b & 0xFF;
        let a = a & 0xFF;
        Self((((((r << 8) | g) << 8) | b) << 8) | a)
    }

    pub fn from_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        let r = (r / 255.0) as u32;
        let g = (g / 255.0) as u32;
        let b = (b / 255.0) as u32;
        let a = (a / 255.0) as u32;
        Self::from_u32(r, g, b, a)
    }

    pub fn get_a(self) -> u32 {
        self.0 & 0xFF000000
    }
    pub fn get_r(self) -> u32 {
        self.0 & 0x000000FF
    }
    pub fn get_g(self) -> u32 {
        self.0 & 0x0000FF00
    }
    pub fn get_b(self) -> u32 {
        self.0 & 0x00FF0000
    }
}

#[repr(C, packed)]
pub struct ColoredRect {
    pub rect: Rect,
    pub color: ColorU32,
    pub i_clip_rect: u32,
    pub padding: [u32; 2],
}

#[repr(C, packed)]
pub struct TexturedRect {
    pub rect: Rect,
    pub uv: Rect,
    pub texture_descriptor: u32,
    pub i_clip_rect: u32,
    pub padding: [u32; 2],
}

#[repr(C)]
pub enum PrimitiveType {
    ColorRect = 0,
    TexturedRect = 1,
    Clip = 2,
    SdfRoundRectangle = 0b100000,
    SdfCircle = 0b100001,
}

// 0b11000000_00000000_00000000_00000000
const CRNR_MAX: u32 = 0b0011;
const CRNR_OFFSET: u32 = 30;
const CRNR_MASK: u32 = CRNR_MAX << CRNR_OFFSET;

// 0b00111111_00000000_00000000_00000000
const TYPE_MAX: u32 = 0b0011_1111;
const TYPE_OFFSET: u32 = 24;
const TYPE_MASK: u32 = TYPE_MAX << TYPE_OFFSET;

// 0b00000000_11111111_11111111_11111111
const INDX_MAX: u32 = 0b1111_1111_1111_1111_1111_1111;
const INDX_OFFSET: u32 = 0;
const INDX_MASK: u32 = INDX_MAX << INDX_OFFSET;

#[repr(C, packed)]
pub struct PrimitiveIndex(u32);

// Bitfields
impl Default for PrimitiveIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimitiveIndex {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn get_corner(&self) -> u32 {
        (self.0 & CRNR_MASK) >> CRNR_OFFSET
    }

    pub fn corner(self, i_corner: u32) -> Self {
        assert!(i_corner <= CRNR_MAX);
        let mut bits = self.0;
        bits &= !CRNR_MASK;
        bits |= i_corner << CRNR_OFFSET;
        PrimitiveIndex(bits)
    }

    pub fn get_i_type(&self) -> u32 {
        (self.0 & TYPE_MASK) >> TYPE_OFFSET
    }

    pub fn i_type(self, i_type: PrimitiveType) -> Self {
        let i_type = i_type as u32;
        assert!(i_type <= TYPE_MAX);
        let mut bits = self.0;
        bits &= !TYPE_MASK;
        bits |= i_type << TYPE_OFFSET;
        PrimitiveIndex(bits)
    }

    pub fn get_index(&self) -> u32 {
        (self.0 & INDX_MASK) >> INDX_OFFSET
    }

    pub fn index(self, index: usize) -> Self {
        assert!((index as u32) <= INDX_MAX);
        let mut bits = self.0;
        bits &= !INDX_MASK;
        bits |= (index as u32) << INDX_OFFSET;
        PrimitiveIndex(bits)
    }
}

pub struct Drawer<'a> {
    vertex_buffer: &'a mut [u8],
    index_buffer: &'a mut [u32],
    vertex_byte_offset: usize,
    index_offset: usize,
    glyph_cache: GlyphCache,
    glyph_atlas_descriptor: u32,
    shape_context: ShapeContext,
}

impl<'a> Drawer<'a> {
    pub fn new(
        vertex_buffer: &'a mut [u8],
        index_buffer: &'a mut [u32],
        glyph_atlas_size: [i32; 2],
        glyph_atlas_descriptor: u32,
    ) -> Self {
        Self {
            vertex_buffer,
            index_buffer,
            vertex_byte_offset: 0,
            index_offset: 0,
            glyph_cache: GlyphCache::new(glyph_atlas_size),
            glyph_atlas_descriptor,
            shape_context: ShapeContext::new(),
        }
    }

    pub fn clear(&mut self) {
        self.vertex_byte_offset = 0;
        self.index_offset = 0;
    }

    pub fn get_vertices(&self) -> &[u8] {
        &self.vertex_buffer[0..self.vertex_byte_offset]
    }

    // Returns the alignment needed on a buffer to hold any kind of primitive
    pub fn get_primitive_alignment() -> usize {
        size_of::<ColoredRect>() * size_of::<TexturedRect>()
    }

    pub fn get_indices(&self) -> &[u32] {
        &self.index_buffer[0..self.index_offset]
    }

    pub fn get_index_offset(&self) -> usize {
        self.index_offset
    }

    pub fn get_glyph_cache_mut(&mut self) -> &mut GlyphCache {
        &mut self.glyph_cache
    }

    pub fn draw_colored_rect(&mut self, rect: Rect, i_clip_rect: u32, color: ColorU32) {
        let i_rect = Self::begin_primitive::<ColoredRect>(&mut self.vertex_byte_offset);
        let vertices = Self::get_primitive_slice::<ColoredRect>(
            self.vertex_buffer,
            self.vertex_byte_offset,
            1,
        );
        let indices = Self::get_index_slice(self.index_buffer, self.index_offset);

        vertices[0] = ColoredRect {
            rect,
            color,
            i_clip_rect,
            padding: [0, 0],
        };

        const CORNERS: [u32; 6] = [0, 1, 2, 2, 3, 0];
        for i_corner in 0..CORNERS.len() {
            indices[i_corner] = PrimitiveIndex::new()
                .index(i_rect)
                .corner(CORNERS[i_corner])
                .i_type(PrimitiveType::ColorRect);
        }

        self.index_offset += CORNERS.len();
        Self::end_primitive::<ColoredRect>(&mut self.vertex_byte_offset, 1);
    }

    pub fn draw_textured_rect(
        &mut self,
        rect: Rect,
        uv: Rect,
        i_clip_rect: u32,
        texture_descriptor: u32,
    ) {
        Self::draw_textured_rect_impl(
            &mut self.vertex_byte_offset,
            self.vertex_buffer,
            &mut self.index_offset,
            self.index_buffer,
            rect,
            uv,
            i_clip_rect,
            texture_descriptor,
        )
    }

    pub fn draw_label(&mut self, face: &Face, label: &str, rect: Rect, i_clip_rect: u32) {
        let mut shaper = self
            .shape_context
            .builder(face.font_ref)
            .size(face.get_size())
            .build();
        shaper.add_str(label);

        let mut cursor: f32 = 0.0;
        shaper.shape_with(|glyph_cluster| {
            for glyph in glyph_cluster.glyphs {
                let glyph_uv = self.glyph_cache.queue_glyph(face, glyph.id);

                let rect = Rect {
                    pos: [cursor + rect.pos[0] + glyph.x, rect.pos[1] + glyph.y],
                    size: [
                        glyph_uv.size[0] * (self.glyph_cache.get_size()[0] as f32),
                        glyph_uv.size[1] * (self.glyph_cache.get_size()[1] as f32),
                    ],
                };

                Self::draw_textured_rect_impl(
                    &mut self.vertex_byte_offset,
                    self.vertex_buffer,
                    &mut self.index_offset,
                    self.index_buffer,
                    rect,
                    glyph_uv,
                    0,
                    self.glyph_atlas_descriptor,
                );

                cursor += glyph.advance;
            }
        });
    }
}

// Impls, functions without self arguments
impl<'a> Drawer<'a> {
    fn begin_primitive<Primitive>(vertex_byte_offset: &mut usize) -> usize {
        let misalignment = *vertex_byte_offset % size_of::<Primitive>();
        if misalignment != 0 {
            *vertex_byte_offset += size_of::<Primitive>() - misalignment;
        }

        assert!(*vertex_byte_offset % size_of::<Primitive>() == 0);

        *vertex_byte_offset / size_of::<Primitive>()
    }

    fn end_primitive<Primitive>(vertex_byte_offset: &mut usize, count: usize) {
        *vertex_byte_offset += count * size_of::<Primitive>();
    }

    fn get_primitive_slice<Primitive>(
        buffer: &mut [u8],
        offset: usize,
        count: usize,
    ) -> &mut [Primitive] {
        let res = unsafe {
            std::slice::from_raw_parts_mut(buffer[offset..].as_ptr() as *mut Primitive, count)
        };
        assert!(res.len() == count);
        res
    }

    fn get_index_slice(indices: &mut [u32], offset: usize) -> &mut [PrimitiveIndex] {
        unsafe {
            std::slice::from_raw_parts_mut(
                indices[offset..].as_ptr() as *mut PrimitiveIndex,
                indices.len() - offset,
            )
        }
    }

    pub fn draw_textured_rect_impl(
        vertex_byte_offset: &mut usize,
        vertex_buffer: &mut [u8],
        index_offset: &mut usize,
        index_buffer: &mut [u32],
        rect: Rect,
        uv: Rect,
        i_clip_rect: u32,
        texture_descriptor: u32,
    ) {
        let i_rect = Self::begin_primitive::<TexturedRect>(vertex_byte_offset);
        let vertices =
            Self::get_primitive_slice::<TexturedRect>(vertex_buffer, *vertex_byte_offset, 1);
        let indices = Self::get_index_slice(index_buffer, *index_offset);

        vertices[0] = TexturedRect {
            rect,
            uv,
            texture_descriptor,
            i_clip_rect,
            padding: [0, 0],
        };

        const CORNERS: [u32; 6] = [0, 1, 2, 2, 3, 0];
        for i_corner in 0..CORNERS.len() {
            indices[i_corner] = PrimitiveIndex::new()
                .index(i_rect)
                .corner(CORNERS[i_corner])
                .i_type(PrimitiveType::TexturedRect);
        }

        *index_offset += CORNERS.len();
        Self::end_primitive::<TexturedRect>(vertex_byte_offset, 1);
    }
}
