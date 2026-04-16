// NOTE: Write-Back cache controller with write-allocate
//       and boundary handling for accesses that cross block boundaries.

use crate::Instruction;
use crate::OPCODE;
use crate::cache::Cache;

#[derive(PartialEq, Debug)]
enum State {
    IDLE,
    COMPARE_TAG,
    WRITE_BACK,
    ALLOCATE,
    //BOUNDARY_FETCH, //To be used to redirect boundary fetch from allocate
}

pub struct CacheController {
    state: State,
    unfinished: usize,
    bytes_done: usize,
    fetch_value: [u8; 4],

    pub hits: usize,
    pub misses: usize,
    pub write_backs: usize,
    pub double_allocations: usize,
}

impl CacheController {
    pub fn new() -> Self {
        CacheController {
            state: State::IDLE,
            unfinished: 0,
            bytes_done: 0,
            fetch_value: [0u8; 4],
            hits: 0,
            misses: 0,
            write_backs: 0,
            double_allocations: 0,
        }
    }

    pub fn cpu_request(
        &mut self,
        inst: &Instruction,
        cache: &mut Cache,
        memory: &mut [u8],
        address_size: u32,
    ) {
        #[cfg_attr(cfg, rustfmt::skip)]
        match inst.opcode {
            OPCODE::NOP     =>  { println!("[NOP]"); return; },
            OPCODE::INVALID =>  { println!("[INVALID]"); return; },
            _ =>  { self.state = State::COMPARE_TAG; }
        }

        loop {
            self.tick(inst, cache, memory, address_size);
            if self.state == State::IDLE {
                break;
            }
        }
    }

    fn compare_tag(&mut self, inst: &Instruction, cache: &mut Cache) {
        let mut is_dirty = false;
        if cache.is_hit(inst.addr, &mut is_dirty) {
            self.hits += 1;

            let boundary = match inst.opcode {
                OPCODE::LB => {
                    let (val_opt, unfinished) = cache.lb(inst.addr);
                    if let Some(b) = val_opt {
                        self.fetch_value[0] = b;
                    }
                    if unfinished != 0 {
                        println!(
                            "[ HIT ] LB addr={} partial: {} byte(s) cross block boundary",
                            inst.addr, unfinished
                        );
                        self.unfinished = unfinished;
                        self.bytes_done = 1 - unfinished;
                        true
                    } else {
                        println!(
                            "[ HIT ] LB addr={} -> val={:#04x}",
                            inst.addr, self.fetch_value[0]
                        );
                        false
                    }
                }

                OPCODE::LH => {
                    let (val_opt, unfinished) = cache.lh(inst.addr);
                    if let Some(bytes) = val_opt {
                        self.fetch_value[0..2].copy_from_slice(&bytes);
                    }
                    if unfinished != 0 {
                        println!(
                            "[ HIT ] LH addr={} partial: {} byte(s) cross block boundary",
                            inst.addr, unfinished
                        );
                        self.unfinished = unfinished;
                        self.bytes_done = 2 - unfinished;
                        true
                    } else {
                        println!(
                            "[ HIT ] LH addr={} -> val={:?}",
                            inst.addr,
                            &self.fetch_value[0..2]
                        );
                        false
                    }
                }

                OPCODE::LW => {
                    let (val_opt, unfinished) = cache.lw(inst.addr);
                    if let Some(bytes) = val_opt {
                        self.fetch_value = bytes;
                    }
                    if unfinished != 0 {
                        println!(
                            "[ HIT ] LW addr={} partial: {} byte(s) cross block boundary",
                            inst.addr, unfinished
                        );
                        self.unfinished = unfinished;
                        self.bytes_done = 4 - unfinished;
                        true
                    } else {
                        println!(
                            "[ HIT ] LW addr={} -> val={:?}",
                            inst.addr, self.fetch_value
                        );
                        false
                    }
                }

                OPCODE::SB => {
                    let unfinished = cache.sb(inst.addr, inst.value[0]);
                    self.fetch_value = inst.value;
                    if unfinished != 0 {
                        println!(
                            "[ HIT ] SB addr={} partial: {} byte(s) cross block boundary",
                            inst.addr, unfinished
                        );
                        self.unfinished = unfinished;
                        self.bytes_done = 1 - unfinished;
                        true
                    } else {
                        println!(
                            "[ HIT ] SB addr={} <- {:#04x} (dirty)",
                            inst.addr, inst.value[0]
                        );
                        false
                    }
                }

                OPCODE::SH => {
                    let unfinished = cache.sh(inst.addr, [inst.value[0], inst.value[1]]);
                    self.fetch_value = inst.value;
                    if unfinished != 0 {
                        println!(
                            "[ HIT ] SH addr={} partial: {} byte(s) cross block boundary",
                            inst.addr, unfinished
                        );
                        self.unfinished = unfinished;
                        self.bytes_done = 2 - unfinished;
                        true
                    } else {
                        println!(
                            "[ HIT ] SH addr={} <- {:?} (dirty)",
                            inst.addr,
                            &inst.value[0..2]
                        );
                        false
                    }
                }

                OPCODE::SW => {
                    let unfinished = cache.sw(inst.addr, inst.value);
                    self.fetch_value = inst.value;
                    if unfinished != 0 {
                        println!(
                            "[ HIT ] SW addr={} partial: {} byte(s) cross block boundary",
                            inst.addr, unfinished
                        );
                        self.unfinished = unfinished;
                        self.bytes_done = 4 - unfinished;
                        true
                    } else {
                        println!("[HIT ] SW addr={} <- {:?} (dirty)", inst.addr, inst.value);
                        false
                    }
                }

                _ => panic!("Unexpected opcode in compare_tag"),
            };

            if boundary {
                self.double_allocations += 1;
                self.state = State::ALLOCATE;
            } else {
                self.state = State::IDLE;
            }
        } else {
            self.misses += 1;
            println!(
                "[ MISS ] addr={} index={} tag={} occupant_dirty={}",
                inst.addr,
                cache.index(inst.addr),
                cache.tag(inst.addr),
                is_dirty
            );
            if is_dirty {
                self.state = State::WRITE_BACK;
            } else {
                self.state = State::ALLOCATE;
            }
        }
    }

    fn write_back(
        &mut self,
        inst: &Instruction,
        cache: &mut Cache,
        memory: &mut [u8],
        address_size: u32,
    ) {
        if let Some(evict_base) = cache.evict_dirty(inst.addr, memory, address_size) {
            self.write_backs += 1;
            println!(
                "[ WB ] Evicted dirty block: base_addr={} tag={} (to make room for addr={})",
                evict_base,
                cache.tag(evict_base),
                inst.addr
            );
        } else {
            println!(
                "[ WB ] Warning: no dirty occupant found at index={}",
                cache.index(inst.addr)
            );
        }
        self.state = State::ALLOCATE;
    }

    fn allocate(
        &mut self,
        inst: &Instruction,
        cache: &mut Cache,
        memory: &[u8],
        address_size: u32,
    ) {
        if self.unfinished == 0 {
            //NOTE: Normal Fetch
            let req_line = cache.request_block(inst.addr, memory, address_size);
            println!(
                "[ALLOC] addr={} block_base={} data={:?}",
                inst.addr,
                inst.addr & !((1u32 << cache.byte_offset_bits) - 1),
                req_line.blocks
            );
            cache.load_block(inst.addr, req_line);
            self.state = State::COMPARE_TAG;
        } else {
            //NOTE: Boundary Fetch
            let next_addr = inst.addr + self.bytes_done as u32;
            let req_line = cache.request_block(next_addr, memory, address_size);
            println!(
                "[ALLOC-2] boundary fetch: next_addr={} block_base={} data={:?}",
                next_addr,
                next_addr & !((1u32 << cache.byte_offset_bits) - 1),
                req_line.blocks
            );

            //NOTE: The fetch_value acted as a buffer between the two fetch to avoid losing
            //      data after first fetch write_back
            let is_store = matches!(inst.opcode, OPCODE::SB | OPCODE::SH | OPCODE::SW);
            if !is_store {
                for i in 0..self.unfinished {
                    self.fetch_value[self.bytes_done + i] = *req_line.blocks.get(i).unwrap_or(&0);
                }
                match inst.opcode {
                    OPCODE::LB => println!(
                        "[ HIT ] LB addr={} -> val={:#04x} (boundary complete)",
                        inst.addr, self.fetch_value[0]
                    ),
                    OPCODE::LH => println!(
                        "[ HIT ] LH addr={} -> val={:?} (boundary complete)",
                        inst.addr,
                        &self.fetch_value[0..2]
                    ),
                    OPCODE::LW => println!(
                        "[ HIT ] LW addr={} -> val={:?} (boundary complete)",
                        inst.addr, self.fetch_value
                    ),
                    _ => {}
                }
            }

            cache.load_block(next_addr, req_line);

            if is_store {
                let tail = &inst.value[self.bytes_done..self.bytes_done + self.unfinished];
                match inst.opcode {
                    OPCODE::SB => {
                        cache.sb(next_addr, tail[0]);
                    }
                    OPCODE::SH => {
                        cache.sh(next_addr, [tail[0], *tail.get(1).unwrap_or(&0)]);
                    }
                    OPCODE::SW => {
                        let mut w = [0u8; 4];
                        for (i, &b) in tail.iter().enumerate() {
                            w[i] = b;
                        }
                        cache.sw(next_addr, w);
                    }
                    _ => {}
                }
                println!(
                    "[ HIT ] {:?} addr={} boundary tail written (dirty)",
                    inst.opcode, inst.addr
                );
            }

            self.unfinished = 0;
            self.bytes_done = 0;
            //FIX: [] Bring it from COMPARE_TAG
            self.state = State::IDLE;
        }
    }

    fn tick(
        &mut self,
        inst: &Instruction,
        cache: &mut Cache,
        memory: &mut [u8],
        address_size: u32,
    ) {
        match self.state {
            State::IDLE => {}
            State::COMPARE_TAG => self.compare_tag(inst, cache),
            State::WRITE_BACK => self.write_back(inst, cache, memory, address_size),
            State::ALLOCATE => self.allocate(inst, cache, memory, address_size),
        }
    }

    pub fn print_stats(&self) {
        let total = self.hits + self.misses;
        let hit_rate = if total > 0 {
            100.0 * self.hits as f64 / total as f64
        } else {
            0.0
        };
        println!("\n                                     ");
        println!("          Cache Statistics           ");
        println!(" ------------------------------------");
        println!("   Hits               : {:<11}  ", self.hits);
        println!("   Misses             : {:<11}  ", self.misses);
        println!("   Write-Backs        : {:<11}  ", self.write_backs);
        println!("   Boundary crossings : {:<11}  ", self.double_allocations);
        println!("   Hit Rate           : {:<10.2}%  ", hit_rate);
        println!("-------------------------------------");
    }
}
