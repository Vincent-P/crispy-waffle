use crate::font::*;
use crate::rect::Rect;

use etagere::BucketedAtlasAllocator;
use nohash_hasher::IntMap;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use swash::scale::{image::Image, Render, ScaleContext, Source, StrikeWith};

pub type GlyphId = swash::GlyphId;
pub type GlyphImage = Image;

// Per-face cache
struct GlyphEntry {
    pub id: GlyphId,
    pub alloc_id: etagere::AllocId,
    pub image: Option<GlyphImage>,
}

struct FaceCache {
    glyphs: Vec<GlyphEntry>,
}

#[derive(Debug)]
pub enum GlyphEvent {
    New(u64, GlyphId, etagere::Rectangle),
    Evicted(etagere::Rectangle),
}

struct AllocMetadata {
    pub rectangle: etagere::Rectangle,
    pub face_hash: u64,
}

// Global cache
pub struct GlyphCache {
    size: [i32; 2],
    atlas: BucketedAtlasAllocator,
    atlas_allocations: HashMap<etagere::AllocId, AllocMetadata>,
    atlas_lru: VecDeque<etagere::AllocId>,
    scale_context: swash::scale::ScaleContext,
    face_caches: IntMap<u64, FaceCache>,
    events: Vec<GlyphEvent>,
}

impl GlyphCache {
    pub fn new(size: [i32; 2]) -> Self {
        Self {
            size,
            atlas: BucketedAtlasAllocator::new(etagere::size2(size[0], size[1])),
            atlas_allocations: HashMap::new(),
            atlas_lru: VecDeque::new(),
            scale_context: ScaleContext::new(),
            face_caches: IntMap::default(),
            events: Vec::new(),
        }
    }

    pub fn queue_char(&mut self, face: &Face, c: char) -> Rect {
        let glyph_id = face.font_ref.charmap().map(c);
        self.queue_glyph(face, glyph_id)
    }

    pub fn queue_glyph(&mut self, face: &Face, glyph_id: GlyphId) -> Rect {
        // Get the face hash
        let face_hash = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            face.font_ref.key.hash(&mut hasher);
            hasher.write_u32(face.size);
            hasher.finish()
        };

        // Find the glyph in the face cache
        if let Some(i_glyph) = self
            .face_caches
            .entry(face_hash)
            .or_insert_with(FaceCache::new)
            .glyphs
            .iter()
            .position(|glyph| glyph.id == glyph_id)
        {
            // The glyph was is already in the cache, put it at the top
            // of the LRU queue
            let glyph_entry = &self.face_caches.get_mut(&face_hash).unwrap().glyphs[i_glyph];

            // Find the position of the glyph in the LRU queue (Very bad)
            let i_lru = self
                .atlas_lru
                .iter()
                .position(|alloc_id| *alloc_id == glyph_entry.alloc_id)
                .unwrap();

            // Remove it (Very bad, shifts all elements after it...)
            self.atlas_lru.remove(i_lru);

            // Put it back at the most recently used slot
            self.atlas_lru.push_back(glyph_entry.alloc_id);

            return rectangle_to_uv(
                self.size,
                self.atlas_allocations[&glyph_entry.alloc_id].rectangle,
            );
        }

        // The glyph was not found, rasterize it and insert it in the cache

        // Render it
        let glyph_image = render_glyph(&mut self.scale_context, face, glyph_id).unwrap();

        // Find free space for the rendered glyph in the glyph atlas
        let mut alloc = self.atlas.allocate(etagere::size2(
            glyph_image.placement.width.try_into().unwrap(),
            glyph_image.placement.height.try_into().unwrap(),
        ));

        // If there isn't enough space in the atlas, evict the least
        // recently used glyphs until there is enough space
        while alloc.is_none() {
            // Find the least recently used allocation
            let lru_alloc = self.atlas_lru.pop_front().unwrap();
            let alloc_data = self.atlas_allocations.get(&lru_alloc).unwrap();

            self.events.push(GlyphEvent::Evicted(alloc_data.rectangle));

            // Remove the allocation from its face cache
            let face_glyph_entries = &mut self
                .face_caches
                .get_mut(&alloc_data.face_hash)
                .unwrap()
                .glyphs;
            let i_glyph = face_glyph_entries
                .iter()
                .position(|entry| entry.alloc_id == lru_alloc)
                .unwrap();
            face_glyph_entries.swap_remove(i_glyph);

            // Remove the allocation from the atlas
            self.atlas.deallocate(lru_alloc);
            self.atlas_allocations.remove(&lru_alloc);

            // Check if there is enough space now
            alloc = self.atlas.allocate(etagere::size2(
                glyph_image.placement.width.try_into().unwrap(),
                glyph_image.placement.height.try_into().unwrap(),
            ));
        }
        let alloc = alloc.unwrap();

        // Add the created allocation on the LRU queue
        self.atlas_lru.push_back(alloc.id);

        // Keep some data about the new allocation
        self.atlas_allocations.insert(
            alloc.id,
            AllocMetadata {
                rectangle: alloc.rectangle,
                face_hash,
            },
        );

        // Add it to its face cache
        let face_glyph_entries = &mut self.face_caches.get_mut(&face_hash).unwrap().glyphs;
        face_glyph_entries.push(GlyphEntry {
            id: glyph_id,
            alloc_id: alloc.id,
            image: Some(glyph_image),
        });

        self.events
            .push(GlyphEvent::New(face_hash, glyph_id, alloc.rectangle));

        rectangle_to_uv(self.size, self.atlas_allocations[&alloc.id].rectangle)
    }

    pub fn process_events<T>(&mut self, callback: T)
    where
        T: Fn(&GlyphEvent, Option<&GlyphImage>),
    {
        for event in self.events.iter() {
            let glyph_image = if let GlyphEvent::New(face_hash, glyph_id, _) = event {
                Some(
                    self.face_caches
                        .get(&face_hash)
                        .unwrap()
                        .glyphs
                        .iter()
                        .find(|glyph_entry| glyph_entry.id == *glyph_id)
                        .unwrap()
                        .image
                        .as_ref()
                        .unwrap(),
                )
            } else {
                None
            };
            callback(event, glyph_image);
        }
        self.events.clear();
    }

    pub fn get_size(&self) -> [i32; 2] {
        self.size
    }
}

impl FaceCache {
    pub fn new() -> Self {
        Self { glyphs: Vec::new() }
    }
}

fn rectangle_to_uv(size: [i32; 2], rectangle: etagere::Rectangle) -> Rect {
    Rect {
        pos: [
            (rectangle.min.x as f32) / (size[0] as f32),
            (rectangle.min.y as f32) / (size[1] as f32),
        ],
        size: [
            ((rectangle.max.x - rectangle.min.x) as f32) / (size[0] as f32),
            ((rectangle.max.y - rectangle.min.y) as f32) / (size[1] as f32),
        ],
    }
}

pub fn render_glyph(
    scale_context: &mut ScaleContext,
    face: &Face,
    glyph_id: GlyphId,
) -> Option<GlyphImage> {
    use swash::zeno::{Format, Vector};

    let x: f32 = 0.0;
    let y: f32 = 0.0;
    let hint: bool = true;

    // Build the scaler
    let mut scaler = scale_context
        .builder(face.font_ref)
        .size(face.get_size())
        .hint(hint)
        .build();

    // Compute the fractional offset-- you'll likely want to quantize this
    // in a real renderer
    let offset = Vector::new(x.fract(), y.fract());
    // Select our source order
    Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ])
    // Select a subpixel format
    .format(Format::Subpixel)
    // Apply the fractional offset
    .offset(offset)
    // Render the image
    .render(&mut scaler, glyph_id)
}