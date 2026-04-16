use rand::Rng;

#[derive(Clone, Debug)]
pub struct CacheLine {
    pub is_valid: bool,
    pub is_dirty: bool,
    pub tag: u32,
    pub blocks: Vec<u8>,
}

impl CacheLine {
    pub fn new(byte_offset: u32) -> Self {
        Self {
            is_valid: false,
            is_dirty: false,
            tag: 0,
            blocks: vec![0; 1 << byte_offset],
        }
    }
}

pub struct Cache {
    pub byte_offset_bits: u32,
    pub index_bits: u32,
    pub tag_bits: u32,
    pub data_size: u32, /* in bytes */
    pub associativity: u32,
    pub cache_lines: Vec<Vec<CacheLine>>,
}

impl Cache {
    pub fn new(byte_offset: u32, index: u32, address_size: u32, associativity: u32) -> Self {
        assert!(
            associativity.is_power_of_two(),
            "Associativity must be power of 2"
        );
        let sets = 1usize << index;
        Self {
            byte_offset_bits: byte_offset,
            index_bits: index,
            tag_bits: address_size - index - byte_offset,
            data_size: (1 << byte_offset) * (1 << index) * associativity,
            associativity: associativity,
            cache_lines: vec![vec![CacheLine::new(byte_offset); sets]; associativity as usize],
        }
    }

    pub fn index(&self, address: u32) -> u32 {
        (address >> self.byte_offset_bits) & ((1u32 << self.index_bits) - 1)
    }

    pub fn tag(&self, address: u32) -> u32 {
        address >> (self.byte_offset_bits + self.index_bits)
    }

    pub fn byte_offset(&self, address: u32) -> u32 {
        address & ((1u32 << self.byte_offset_bits) - 1)
    }

    pub fn lb(&self, address: u32) -> (Option<u8>, usize) {
        let set = self.index(address) as usize;
        let tag = self.tag(address);
        let offset = self.byte_offset(address) as usize;

        for way in &self.cache_lines {
            let line = &way[set];
            if line.tag == tag && line.is_valid {
                return match line.blocks.get(offset) {
                    Some(val) => (Some(*val), 0),
                    None => (Some(0), 1),
                };
            }
        }
        (None, 1)
    }

    pub fn lh(&self, address: u32) -> (Option<[u8; 2]>, usize) {
        let set = self.index(address) as usize;
        let tag = self.tag(address);
        let offset = self.byte_offset(address) as usize;

        for way in &self.cache_lines {
            let line = &way[set];
            if line.tag == tag && line.is_valid {
                let mut bytes = [0u8; 2];
                let mut unfinished = 2usize;
                for i in 0..2 {
                    match line.blocks.get(offset + i) {
                        Some(val) => {
                            bytes[i] = *val;
                            unfinished -= 1;
                        }
                        None => break,
                    }
                }
                return (Some(bytes), unfinished);
            }
        }
        (None, 2)
    }

    pub fn lw(&self, address: u32) -> (Option<[u8; 4]>, usize) {
        let set = self.index(address) as usize;
        let tag = self.tag(address);
        let offset = self.byte_offset(address) as usize;

        for way in &self.cache_lines {
            let line = &way[set];
            if line.tag == tag && line.is_valid {
                let mut bytes = [0u8; 4];
                let mut unfinished = 4usize;
                for i in 0..4 {
                    match line.blocks.get(offset + i) {
                        Some(val) => {
                            bytes[i] = *val;
                            unfinished -= 1;
                        }
                        None => break,
                    }
                }
                return (Some(bytes), unfinished);
            }
        }
        (None, 4)
    }

    pub fn sb(&mut self, address: u32, byte: u8) -> usize {
        let set = self.index(address) as usize;
        let tag = self.tag(address);
        let offset = self.byte_offset(address) as usize;

        for way in &mut self.cache_lines {
            let line = &mut way[set];
            if line.tag == tag && line.is_valid {
                line.blocks[offset] = byte;
                line.is_dirty = true;
                return 0;
            }
        }
        1
    }

    pub fn sh(&mut self, address: u32, half_word: [u8; 2]) -> usize {
        let set = self.index(address) as usize;
        let tag = self.tag(address);
        let offset = self.byte_offset(address) as usize;
        let mut unfinished = 2usize;

        for way in &mut self.cache_lines {
            let line = &mut way[set];
            if line.tag == tag && line.is_valid {
                for i in 0..2 {
                    match line.blocks.get_mut(offset + i) {
                        Some(b) => {
                            *b = half_word[i];
                            unfinished -= 1;
                            line.is_dirty = true;
                        }
                        None => break,
                    }
                }
                return unfinished;
            }
        }
        unfinished
    }

    pub fn sw(&mut self, address: u32, word: [u8; 4]) -> usize {
        let set = self.index(address) as usize;
        let tag = self.tag(address);
        let offset = self.byte_offset(address) as usize;
        let mut unfinished = 4usize;

        for way in &mut self.cache_lines {
            let line = &mut way[set];
            if line.tag == tag && line.is_valid {
                for i in 0..4 {
                    match line.blocks.get_mut(offset + i) {
                        Some(b) => {
                            *b = word[i];
                            unfinished -= 1;
                            line.is_dirty = true;
                        }
                        None => break,
                    }
                }
                return unfinished;
            }
        }
        unfinished
    }

    pub fn request_block(&self, address: u32, memory: &[u8], address_size: u32) -> CacheLine {
        let block_aligned_address = address & !((1u32 << self.byte_offset_bits) - 1);
        let mut block = Vec::new();
        for i in 0u32..(1 << self.byte_offset_bits) {
            block.push(memory[((block_aligned_address + i) as usize) % address_size as usize]);
        }
        CacheLine {
            is_valid: true,
            is_dirty: false,
            tag: self.tag(address),
            blocks: block,
        }
    }

    pub fn load_block(&mut self, address: u32, cache_line: CacheLine) {
        let mut rng = rand::thread_rng();
        let index = self.index(address) as usize;
        let way = rng.gen_range(0..self.associativity as usize);
        self.cache_lines[way][index] = cache_line;
    }

    pub fn write_block(
        &self,
        address: u32,
        memory: &mut [u8],
        address_size: u32,
        cache_line: &CacheLine,
    ) {
        let block_aligned_address = address & !((1u32 << self.byte_offset_bits) - 1);

        for i in 0u32..(1 << self.byte_offset_bits) {
            memory[((block_aligned_address + i) as usize) % address_size as usize] =
                cache_line.blocks[i as usize];
        }
    }

    pub fn is_hit(&self, address: u32, is_dirty: &mut bool) -> bool {
        let set = self.index(address) as usize;
        for way in &self.cache_lines {
            let line = &way[set];
            if line.is_valid && line.tag == self.tag(address) {
                *is_dirty = line.is_dirty;
                return true;
            }
        }

        // On a miss, check if the current occupant at this set needs writeback.
        for way in &self.cache_lines {
            let line = &way[set];
            if line.is_valid && line.is_dirty {
                *is_dirty = true;
                return false; // miss with dirty eviction required
            }
        }

        *is_dirty = false;
        false
    }

    // Write back any dirty line at address's set to memory, mark it clean.
    // Returns the evicted block's base address for logging, or None if clean.
    pub fn evict_dirty(
        &mut self,
        address: u32,
        memory: &mut [u8],
        address_size: u32,
    ) -> Option<u32> {
        let set = self.index(address) as usize;
        for way in &mut self.cache_lines {
            let line = &mut way[set];
            if line.is_valid && line.is_dirty {
                let evict_base = (line.tag << (self.byte_offset_bits + self.index_bits))
                    | ((set as u32) << self.byte_offset_bits);
                for i in 0u32..(1 << self.byte_offset_bits) {
                    memory[((evict_base + i) as usize) % address_size as usize] =
                        line.blocks[i as usize];
                }
                line.is_dirty = false;
                return Some(evict_base);
            }
        }
        None
    }

    pub fn dump(&self) {
        println!("\n   Cache Dump");
        println!(
            "  {:>4}  {:>4}  {:>5}  {:>5}  {:>6}  Data",
            "Way", "Set", "Valid", "Dirty", "Tag"
        );
        println!(" -------------------------------------------------------------------");
        let mut any = false;
        for (w, way) in self.cache_lines.iter().enumerate() {
            for (s, line) in way.iter().enumerate() {
                if line.is_valid {
                    println!(
                        "  {:>4}  {:>4}  {:>5}  {:>5}  {:>6}  {:?}",
                        w, s, line.is_valid, line.is_dirty, line.tag, line.blocks
                    );
                    any = true;
                }
            }
        }
        if !any {
            println!("  (all lines invalid / empty)");
        }
        println!("--------------------------------------------------------------------");
    }
}
