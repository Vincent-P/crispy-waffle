enum Entry<T> {
    Empty(Option<u32>),
    Filled(T),
}

struct Metadata {
    flags: u32,
}

pub struct Handle<T> {
    index: u32,
    generation: u32,
    marker: std::marker::PhantomData<T>,
}

// Clone and Copy need to be impl manually because of PhantomData
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Handle<T> {
        Handle {
            index: self.index,
            generation: self.generation,
            marker: std::marker::PhantomData,
        }
    }
}
impl<T> Copy for Handle<T> {}

pub struct Pool<T> {
    values: Vec<(Metadata, Entry<T>)>,
    freelist_head: Option<u32>,
    size: u32,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Pool<T> {
    pub fn new() -> Self {
        Pool {
            values: vec![],
            freelist_head: None,
            size: 0,
        }
    }

    pub fn with_capacity(capacity: u32) -> Self {
        let mut pool = Pool {
            values: Vec::with_capacity(capacity as usize),
            freelist_head: None,
            size: 0,
        };
        pool.init_freelist(0);
        pool
    }

    fn init_freelist(&mut self, old_capacity: usize) {
        let new_capacity = self.values.len();
        for i in old_capacity..new_capacity {
            self.values[i].0.flags = 0;
            self.values[i].1 = Entry::Empty(Some(i as u32 + 1));
        }
        self.values[new_capacity - 1].1 = Entry::Empty(None);
    }

    pub fn add(&mut self, value: T) -> Handle<T> {
        if self.freelist_head.is_none() {
            let old_capacity = self.values.len();
            let new_capacity = 2 * old_capacity.max(1);
            self.values
                .resize_with(new_capacity, || (Metadata { flags: 0 }, Entry::Empty(None)));
            self.init_freelist(old_capacity);
            self.freelist_head = self.values[old_capacity].1.as_empty().unwrap();
        }

        let i_element = self.freelist_head.unwrap();
        let (metadata, element) = &mut self.values[i_element as usize];

        self.freelist_head = element.as_empty().unwrap();

        *element = Entry::Filled(value);

        assert!(!metadata.get_is_occupied());
        metadata.set_is_occupied(true);
        assert!(metadata.get_is_occupied());

        self.size += 1;

        Handle {
            index: i_element,
            generation: metadata.get_generation(),
            marker: Default::default(),
        }
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        let (metadata, element) = &mut self.values[handle.index as usize];
        assert!(metadata.get_is_occupied());
        assert!(handle.generation == metadata.get_generation());
        return element.as_filled_mut().unwrap();
    }

    pub fn get(&self, handle: Handle<T>) -> &T {
        let (metadata, element) = &self.values[handle.index as usize];
        assert!(metadata.get_is_occupied());
        assert!(handle.generation == metadata.get_generation());
        return element.as_filled().unwrap();
    }

    pub fn remove(&mut self, handle: Handle<T>) {
        let (metadata, element) = &mut self.values[handle.index as usize];
        assert!(metadata.get_is_occupied());
        assert!(handle.generation == metadata.get_generation());

        metadata.set_is_occupied(false);
        assert!(!metadata.get_is_occupied());
        metadata.set_generation(handle.generation + 1);

        *element = Entry::Empty(self.freelist_head);
        self.freelist_head = Some(handle.index);

        self.size -= 1;
    }
}

const OCCUPIED_MASK: u32 = 0x80000000;
const GENERATION_MASK: u32 = 0x7FFFFFFF;

impl Metadata {
    pub fn get_is_occupied(&self) -> bool {
        (self.flags & OCCUPIED_MASK) != 0
    }

    pub fn set_is_occupied(&mut self, is_occupied: bool) {
        if is_occupied {
            self.flags |= OCCUPIED_MASK
        } else {
            self.flags &= !OCCUPIED_MASK
        };
    }

    pub fn get_generation(&self) -> u32 {
        self.flags & GENERATION_MASK
    }

    pub fn set_generation(&mut self, generation: u32) {
        self.flags = (self.flags & OCCUPIED_MASK) | (generation & GENERATION_MASK);
    }
}

impl<T> Entry<T> {
    pub fn as_empty(&self) -> Option<Option<u32>> {
        match self {
            Entry::Empty(next) => Some(*next),
            _ => None,
        }
    }
    pub fn as_filled_mut(&mut self) -> Option<&mut T> {
        match self {
            Entry::Filled(inner) => Some(inner),
            _ => None,
        }
    }
    pub fn as_filled(&self) -> Option<&T> {
        match self {
            Entry::Filled(inner) => Some(inner),
            _ => None,
        }
    }
}
