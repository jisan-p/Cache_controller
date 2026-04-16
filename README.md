# Cache Controller Simulator

An interactive, step-by-step cache simulator written in **Rust**. It models a write-back, N-way set-associative cache and supports RISC-style load/store instructions.

---

## FSM Diagram

![Cache Controller FSM](https://github.com/user-attachments/assets/257faa7e-8938-4f14-ac71-37bb6f756107)

---

## Cache Configuration

| Parameter      | Value      |
|----------------|------------|
| Associativity  | 2-way      |
| Sets           | 8          |
| Block size     | 4 bytes    |
| Total size     | 64 bytes   |
| Address space  | 256 bytes  |

---

## Build & Run

> Requires [Rust / Cargo](https://www.rust-lang.org/tools/install)

```bash
cargo run <input_file>
```

**Example:**
```bash
cargo run input.txt
```

---

## Input File Format

Each line is one instruction:

```
lw  <address>
lh  <address>
lb  <address>
sw  <address> <value>
sh  <address> <value>
sb  <address> <value>
nop
```

**Example (`input.txt`):**
```
lw 0
sw 10 255
lw 8
nop
```

---

## Interactive Commands

Once running, step through instructions one at a time:

| Command | Description                        |
|---------|------------------------------------|
| `s`     | Execute next instruction           |
| `x`     | Print hit / miss / writeback stats |
| `d`     | Dump all valid cache lines         |
| `h`     | Show help                          |
| `q`     | Quit (prints final stats + dump)   |

---

## How It Works

1. **Hit** — tag matches a valid line → data read/written directly
2. **Miss (clean)** — new block fetched from memory and loaded into cache
3. **Miss (dirty)** — dirty occupant written back to memory first, then new block loaded
4. **Write-back** — modified lines are only flushed to memory on eviction

Replacement uses **random selection** within a set.

---

## Authors

- **Mehedul Hasan Prodhan** — 230041116
- **Najmus Sakib** — 230041149
- **Nuhiat Arefin** — 230041147

> Submitted: 16 April 2026
