use crate::rect::Rect;
use std::mem::size_of;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct ColorU32(pub u32);

impl ColorU32 {
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
}

impl<'a> Drawer<'a> {
    pub fn new(vertex_buffer: &'a mut [u8], index_buffer: &'a mut [u32]) -> Self {
        Self {
            vertex_buffer,
            index_buffer,
            vertex_byte_offset: 0,
            index_offset: 0,
        }
    }

    pub fn clear(&mut self) {
        self.vertex_byte_offset = 0;
        self.index_offset = 0;
    }

    pub fn draw_colored_rect(&mut self, rect: Rect, i_clip_rect: u32, color: ColorU32) {
        if color.get_a() == 0 {
            return;
        }

        let misalignment = self.vertex_byte_offset % size_of::<ColoredRect>();
        if misalignment != 0 {
            self.vertex_byte_offset += size_of::<ColoredRect>() - misalignment;
        }

        assert!(self.vertex_byte_offset % size_of::<ColoredRect>() == 0);

        let i_rect = self.vertex_byte_offset / size_of::<ColoredRect>();

        let vertices = unsafe {
            std::slice::from_raw_parts_mut(
                self.vertex_buffer[self.vertex_byte_offset..].as_ptr() as *mut ColoredRect,
                1,
            )
        };

        let indices = unsafe {
            std::slice::from_raw_parts_mut(
                self.index_buffer[self.index_offset..].as_ptr() as *mut PrimitiveIndex,
                self.index_buffer.len() - self.index_offset,
            )
        };

        vertices[0] = ColoredRect {
            rect,
            color,
            i_clip_rect,
            padding: [0, 0],
        };
        self.vertex_byte_offset += size_of::<ColoredRect>();

        const CORNERS: [u32; 6] = [0, 1, 2, 2, 3, 0];
        for i_corner in 0..CORNERS.len() {
            indices[i_corner] = PrimitiveIndex::new()
                .index(i_rect)
                .corner(CORNERS[i_corner])
                .i_type(PrimitiveType::ColorRect);
        }
        self.index_offset += CORNERS.len();
    }

    pub fn get_vertex_buffer(&self) -> &[u8] {
        self.vertex_buffer
    }

    pub fn get_index_buffer(&self) -> &[u32] {
        self.index_buffer
    }

    pub fn get_index_offset(&self) -> usize {
        self.index_offset
    }
}
