use crate::font::*;
use crate::glyph_cache::*;
use crate::rect::Rect;
use std::mem::size_of;
use swash::shape::ShapeContext;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct ColorU32(pub u32);

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct ColoredRect {
    pub rect: Rect,
    pub color: ColorU32,
    pub i_clip_rect: u32,
    pub border_radius: f32,
    pub padding: u32,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct TexturedRect {
    pub rect: Rect,
    pub uv: Rect,
    pub texture_descriptor: u32,
    pub i_clip_rect: u32,
    pub border_radius: f32,
    pub base_color: ColorU32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub enum PrimitiveType {
    ColorRect = 0,
    TexturedRect = 1,
    Clip = 2,
    SdfCircle = 0b100000,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct PrimitiveIndex(u32);

pub struct TextGlyph {
    placement: swash::zeno::Placement,
    atlas_pos: Option<[i32; 2]>,
    offsets: [f32; 2],
    advance: f32,
}

pub struct TextCluster {
    glyphs: Vec<TextGlyph>,
}

pub struct TextRun {
    metrics: swash::Metrics,
    glyph_clusters: Vec<TextCluster>,
    glyph_count: usize,
}

pub struct TextLayout {
    size: [f32; 2],
    glyph_positions: Vec<[f32; 2]>,
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

    pub fn draw_colored_rect(&mut self, rect: ColoredRect) {
        Self::draw_colored_rects_impl(
            &mut self.vertex_byte_offset,
            self.vertex_buffer,
            &mut self.index_offset,
            self.index_buffer,
            &[rect],
        )
    }

    pub fn draw_textured_rect(&mut self, rect: TexturedRect) {
        Self::draw_textured_rects_impl(
            &mut self.vertex_byte_offset,
            self.vertex_buffer,
            &mut self.index_offset,
            self.index_buffer,
            &[rect],
        )
    }

    pub fn shape_text(&mut self, face: &Face, text: &str) -> TextRun {
        let mut shaper = self
            .shape_context
            .builder(face.font_ref)
            .size(face.get_size())
            .build();
        shaper.add_str(text);
        let mut text_run = TextRun {
            metrics: shaper.metrics(),
            glyph_clusters: Vec::with_capacity(8),
            glyph_count: 0,
        };

        shaper.shape_with(|glyph_cluster| {
            let mut cluster = TextCluster {
                glyphs: Vec::with_capacity(glyph_cluster.glyphs.len()),
            };
            for glyph in glyph_cluster.glyphs {
                let (atlas_pos, glyph_image) = self.glyph_cache.queue_glyph(face, glyph.id);

                cluster.glyphs.push(TextGlyph {
                    placement: glyph_image.placement,
                    atlas_pos,
                    offsets: [glyph.x, glyph.y],
                    advance: glyph.advance,
                });
            }

            text_run.glyph_count += cluster.glyphs.len();
            text_run.glyph_clusters.push(cluster);
        });

        text_run
    }

    pub fn layout_text(text_run: &TextRun, width_constraint: Option<f32>) -> TextLayout {
        let mut layout = TextLayout {
            size: [0.0, 0.0],
            glyph_positions: Vec::new(),
        };

        let line_height =
            text_run.metrics.ascent + text_run.metrics.descent + text_run.metrics.leading;

        let mut cursor_x: f32 = 0.0;
        let mut cursor_y: f32 = text_run.metrics.ascent;

        for cluster in &text_run.glyph_clusters {
            for glyph in &cluster.glyphs {
                let glyph_top_left = [
                    cursor_x + glyph.offsets[0] + (glyph.placement.left as f32),
                    cursor_y + glyph.offsets[1] - (glyph.placement.top as f32),
                ];

                let glyph_size = [glyph.placement.width as f32, glyph.placement.height as f32];

                cursor_x += glyph.advance;

                // Break to a new line if the current glyph is outside the constraint
                match width_constraint {
                    Some(constraint) if glyph_top_left[0] + glyph_size[0] > constraint => {
                        layout.size[0] = layout.size[0].max(cursor_x);
                        cursor_x = 0.0;
                        cursor_y += line_height;
                    }
                    _ => {}
                }

                layout.glyph_positions.push(glyph_top_left);
            }
        }

        layout.size[0] = layout.size[0].max(cursor_x).ceil();
        layout.size[1] = (cursor_y + text_run.metrics.descent).ceil();

        layout
    }

    pub fn shape_and_layout_text(&mut self, face: &Face, text: &str) -> (TextRun, TextLayout) {
        let text_run = self.shape_text(face, text);
        let text_layout = Self::layout_text(&text_run, None);
        (text_run, text_layout)
    }

    pub fn draw_label(
        &mut self,
        face: &Face,
        label: &str,
        rect: Rect,
        i_clip_rect: u32,
        color: ColorU32,
    ) {
        let text_run = self.shape_text(face, label);
        let text_layout = Self::layout_text(&text_run, None);

        let centered_text = Rect::center(rect, text_layout.size);
        self.draw_text_run(
            &text_run,
            &text_layout,
            centered_text.pos,
            i_clip_rect,
            color,
        );
    }

    pub fn draw_text_run(
        &mut self,
        text_run: &TextRun,
        text_layout: &TextLayout,
        pos: [f32; 2],
        i_clip_rect: u32,
        color: ColorU32,
    ) {
        let mut rects = Vec::new();

        let mut i_glyph = 0;
        for cluster in &text_run.glyph_clusters {
            for glyph in &cluster.glyphs {
                // Glyphs that don't have a position in the atlas are zero-sized glyphs
                if let Some(atlas_pos) = glyph.atlas_pos {
                    let glyph_position = text_layout.glyph_positions[i_glyph];

                    let rect = Rect {
                        pos: [pos[0] + glyph_position[0], pos[1] + glyph_position[1]],
                        size: [glyph.placement.width as f32, glyph.placement.height as f32],
                    };

                    let glyph_uv = Rect {
                        pos: [
                            (atlas_pos[0] as f32) / (self.glyph_cache.get_size()[0] as f32),
                            (atlas_pos[1] as f32) / (self.glyph_cache.get_size()[1] as f32),
                        ],
                        size: [
                            (rect.size[0] as f32) / (self.glyph_cache.get_size()[0] as f32),
                            (rect.size[1] as f32) / (self.glyph_cache.get_size()[1] as f32),
                        ],
                    };

                    rects.push(
                        TexturedRect::new(rect)
                            .uv(glyph_uv)
                            .i_clip_rect(i_clip_rect)
                            .texture_descriptor(self.glyph_atlas_descriptor)
                            .base_color(color),
                    );
                }
                i_glyph += 1;
            }
        }

        Self::draw_textured_rects_impl(
            &mut self.vertex_byte_offset,
            self.vertex_buffer,
            &mut self.index_offset,
            self.index_buffer,
            &rects,
        );
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

    pub fn draw_textured_rects_impl(
        vertex_byte_offset: &mut usize,
        vertex_buffer: &mut [u8],
        index_offset: &mut usize,
        index_buffer: &mut [u32],
        // position, uv, i_clip_rect, texture_descriptor
        rects: &[TexturedRect],
    ) {
        let i_first_rect = Self::begin_primitive::<TexturedRect>(vertex_byte_offset);
        let vertices = Self::get_primitive_slice::<TexturedRect>(
            vertex_buffer,
            *vertex_byte_offset,
            rects.len(),
        );
        let indices = Self::get_index_slice(index_buffer, *index_offset);

        const CORNERS: [u32; 6] = [0, 1, 2, 0, 2, 3];
        for (i_rect, textured_rect) in rects.iter().enumerate() {
            vertices[i_rect] = *textured_rect;

            for i_corner in 0..CORNERS.len() {
                indices[i_rect * CORNERS.len() + i_corner] = PrimitiveIndex::new()
                    .index(i_first_rect + i_rect)
                    .corner(CORNERS[i_corner])
                    .i_type(PrimitiveType::TexturedRect);
            }
        }

        *index_offset += rects.len() * CORNERS.len();
        Self::end_primitive::<TexturedRect>(vertex_byte_offset, rects.len());
    }

    pub fn draw_colored_rects_impl(
        vertex_byte_offset: &mut usize,
        vertex_buffer: &mut [u8],
        index_offset: &mut usize,
        index_buffer: &mut [u32],
        // position, uv, i_clip_rect, texture_descriptor
        rects: &[ColoredRect],
    ) {
        let i_first_rect = Self::begin_primitive::<ColoredRect>(vertex_byte_offset);
        let vertices = Self::get_primitive_slice::<ColoredRect>(
            vertex_buffer,
            *vertex_byte_offset,
            rects.len(),
        );
        let indices = Self::get_index_slice(index_buffer, *index_offset);

        const CORNERS: [u32; 6] = [0, 1, 2, 0, 2, 3];
        for (i_rect, colored_rect) in rects.iter().enumerate() {
            vertices[i_rect] = *colored_rect;

            for i_corner in 0..CORNERS.len() {
                indices[i_rect * CORNERS.len() + i_corner] = PrimitiveIndex::new()
                    .index(i_first_rect + i_rect)
                    .corner(CORNERS[i_corner])
                    .i_type(PrimitiveType::ColorRect);
            }
        }

        *index_offset += rects.len() * CORNERS.len();
        Self::end_primitive::<ColoredRect>(vertex_byte_offset, rects.len());
    }
}

impl TextRun {
    pub fn metrics(&self) -> swash::Metrics {
        self.metrics
    }
}

impl TextLayout {
    pub fn size(&self) -> [f32; 2] {
        self.size
    }
}

impl ColorU32 {
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        let r = r as u32;
        let g = g as u32;
        let b = b as u32;
        let a = a as u32;
        Self((((((a << 8) | b) << 8) | g) << 8) | r)
    }

    pub fn from_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        let r = (r * 255.0) as u8;
        let g = (g * 255.0) as u8;
        let b = (b * 255.0) as u8;
        let a = (a * 255.0) as u8;
        Self::from_u8(r, g, b, a)
    }

    pub fn red() -> Self {
        Self::from_f32(1.0, 0.0, 0.0, 1.0)
    }

    pub fn green() -> Self {
        Self::from_f32(0.0, 1.0, 0.0, 1.0)
    }

    pub fn blue() -> Self {
        Self::from_f32(0.0, 0.0, 1.0, 1.0)
    }

    pub fn cyan() -> Self {
        Self::from_f32(0.0, 1.0, 1.0, 1.0)
    }

    pub fn magenta() -> Self {
        Self::from_f32(1.0, 0.0, 1.0, 1.0)
    }

    pub fn yellow() -> Self {
        Self::from_f32(1.0, 1.0, 0.0, 1.0)
    }

    pub fn greyscale(intensity: u8) -> Self {
        Self::from_u8(intensity, intensity, intensity, 255)
    }

    pub fn r(self) -> u32 {
        self.0 & 0x000000FF
    }
    pub fn g(self) -> u32 {
        self.0 & 0x0000FF00
    }
    pub fn b(self) -> u32 {
        self.0 & 0x00FF0000
    }
    pub fn a(self) -> u32 {
        self.0 & 0xFF000000
    }

    pub fn r_f32(self) -> f32 {
        (self.r() as f32) / 255.0
    }
    pub fn g_f32(self) -> f32 {
        (self.g() as f32) / 255.0
    }
    pub fn b_f32(self) -> f32 {
        (self.b() as f32) / 255.0
    }
    pub fn a_f32(self) -> f32 {
        (self.a() as f32) / 255.0
    }
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

// I hate Rust.
impl ColoredRect {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            color: ColorU32::magenta(),
            i_clip_rect: !0u32,
            border_radius: 0.0,
            padding: 0,
        }
    }

    pub fn rect(mut self, rect: Rect) -> Self {
        self.rect = rect;
        self
    }

    pub fn color(mut self, color: ColorU32) -> Self {
        self.color = color;
        self
    }
    pub fn i_clip_rect(mut self, i_clip_rect: u32) -> Self {
        self.i_clip_rect = i_clip_rect;
        self
    }

    pub fn border_radius(mut self, border_radius: f32) -> Self {
        self.border_radius = border_radius;
        self
    }
}

// I hate Rust.
impl TexturedRect {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            uv: Rect::default(),
            texture_descriptor: !0u32,
            i_clip_rect: !0u32,
            border_radius: 0.0,
            base_color: ColorU32::greyscale(0xFF),
        }
    }

    pub fn uv(mut self, uv: Rect) -> Self {
        self.uv = uv;
        self
    }

    pub fn texture_descriptor(mut self, texture_descriptor: u32) -> Self {
        self.texture_descriptor = texture_descriptor;
        self
    }

    pub fn i_clip_rect(mut self, i_clip_rect: u32) -> Self {
        self.i_clip_rect = i_clip_rect;
        self
    }

    pub fn border_radius(mut self, border_radius: f32) -> Self {
        self.border_radius = border_radius;
        self
    }

    pub fn base_color(mut self, base_color: ColorU32) -> Self {
        self.base_color = base_color;
        self
    }
}
