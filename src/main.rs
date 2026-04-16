use std::env;
use std::fs;
use std::io::{self, Write};

mod cache;
mod cache_controller;

use cache::Cache;
use cache_controller::CacheController;

const ADDRESS_SIZE: u32 = 1 << 8;

#[derive(Debug)]
enum OPCODE {
    LW,
    LH,
    LB,
    SW,
    SH,
    SB,
    NOP,
    INVALID,
}

#[derive(Debug)]
struct Instruction {
    opcode: OPCODE,
    value: [u8; 4],
    addr: u32,
}

fn parse_line(line: &str) -> Instruction {
    let mut parts = line.split_whitespace();

    #[cfg_attr(cfg, rustfmt::skip)]
    let opcode = match parts.next() {
        Some(op) => op,
        None => {
            return Instruction { opcode: OPCODE::INVALID, addr: 0, value: [0u8; 4] };
        }
    };

    //TODO: [ ] Remove unwrap and do it properly

    #[cfg_attr(cfg, rustfmt::skip)]
    let address = match parts.next() {
        Some(addr) => addr.parse::<u32>().unwrap(),
        None => 0,
    };

    #[cfg_attr(cfg, rustfmt::skip)]
    let value = match parts.next() {
        Some(val) => val.parse::<u32>().unwrap(),
        None => 0,
    };

    let mut bytes = [0u8; 4];

    bytes[0] = (value >> 24) as u8;
    bytes[1] = (value >> 16) as u8;
    bytes[2] = (value >> 8) as u8;
    bytes[3] = value as u8;

    #[cfg_attr(cfg, rustfmt::skip)]
    match opcode {
        "lw"    => Instruction { opcode: OPCODE::LW, addr: address, value: [0u8; 4] },
        "lh"    => Instruction { opcode: OPCODE::LH, addr: address, value: [0u8; 4] },
        "lb"    => Instruction { opcode: OPCODE::LB, addr: address, value: [0u8; 4] },
        "sw"    => Instruction { opcode: OPCODE::SW, addr: address, value: bytes },
        "sh"    => Instruction { opcode: OPCODE::SH, addr: address, value: bytes },
        "sb"    => Instruction { opcode: OPCODE::SB, addr: address, value: bytes },
        "nop"   => Instruction { opcode: OPCODE::NOP, addr: 0, value: [0u8; 4] },
        _       => Instruction { opcode: OPCODE::INVALID, addr: 0, value: [0u8; 4] }
    }
}

fn parse_file(contents: &str) -> Vec<Instruction> {
    contents.lines().map(parse_line).collect()
}

fn flush_prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    buf.trim().to_string()
}

fn print_help() {
    println!("\n Commands:");
    println!(" s : Run 1 instruction");
    println!(" x : Print hit/miss/writeback statistics");
    println!(" d : Dump valid cache lines");
    println!(" q : Exit (prints final stats + dump)");
    println!(" h : Show this message");
}

fn format_msg() {
    println!("Input format: ");
    println!("lw/lh/lb [Address] ");
    println!("sw/sh/sb [Address] [store value]");
    println!("nop //To simulate a normal instruction without memory access");
    println!("Something invalid will show INVALID");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Format: ./a.out [asm.S]");
        format_msg();
        return Ok(());
    }

    let contents = fs::read_to_string(&args[1])?;
    let instructions = parse_file(&contents);
    println!(
        "Loaded {} instruction(s) from '{}'.",
        instructions.len(),
        &args[1]
    );

    //TODO: [] Make it user reconfigurable
    // Cache config: 2-bit byte offset
    //               3-bit index
    //               assosiativity = 2

    let byte_offset = 2u32;
    let index_bits = 3u32;
    let associativity = 2u32;

    println!(
        "Cache: {}-way, {} sets, {}-byte blocks ({} bytes total)",
        associativity,
        1 << index_bits,
        1 << byte_offset,
        (1 << byte_offset) * (1 << index_bits) * associativity
    );

    let make_cache = || Cache::new(byte_offset, index_bits, ADDRESS_SIZE, associativity);

    let mut cache = make_cache();
    let mut memory = vec![0u8; ADDRESS_SIZE as usize];
    let mut cache_controller = CacheController::new();
    let mut pc = 0usize;

    print_help();

    loop {
        let remaining = instructions.len().saturating_sub(pc);
        let raw = flush_prompt(&format!(
            "\n[{}/{}  {} left]> ",
            pc,
            instructions.len(),
            remaining
        ));
        let mut tokens = raw.split_whitespace();

        match tokens.next().unwrap_or("") {
            "s" => {
                if pc >= instructions.len() {
                    println!("  No more instructions.");
                } else {
                    println!(
                        "  Executing [{}/{}]: {:?}",
                        pc,
                        instructions.len() - 1,
                        instructions[pc]
                    );
                    cache_controller.cpu_request(
                        &instructions[pc],
                        &mut cache,
                        &mut memory,
                        ADDRESS_SIZE,
                    );
                    pc += 1;
                }
            }

            "x" => cache_controller.print_stats(),

            "d" => cache.dump(),

            "h" => print_help(),

            "q" => break,

            "" => {}
            other => println!("  Unknown command '{}'. Type 'help'.", other),
        }
    }

    cache_controller.print_stats();
    cache.dump();
    Ok(())
}
