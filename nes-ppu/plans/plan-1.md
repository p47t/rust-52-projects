# Plan: Fix blargg ppu_vbl_nmi Tests 02, 05, 06, 07, 08, 10

## Context

The nes-ppu emulator passes blargg tests 01, 03, 04, 09 but fails 02, 05, 06, 07, 08, 10. After extensive research into real NES PPU timing (nesdev wiki), the root causes are:

1. **No sub-cycle read/write timing** — Our catch-up ticks the PPU to `cpu_cycles * 3` (end of CPU cycle), but on real hardware reads latch data EARLY in the cycle and writes take effect LATE. This shifts register-read timing by ~2 PPU ticks.
2. **No VBlank suppression** — Reading $2002 at the exact VBlank set cycle should suppress VBlank entirely (tests 02, 06).
3. **Over-aggressive NMI cancellation** — Falling-edge NMI cancellation from register writes cancels committed NMIs (test 08) and interferes with write-delayed NMIs near pre-render clear (test 07).
4. **Wrong odd-frame skip timing** — Skip happens at frame start (scanline 0) instead of pre-render end (scanline 261, dot 339→0,0) (test 10).

### Key NES Hardware Facts (from nesdev wiki)
- VBlank flag set at scanline 241, dot 1; cleared at scanline 261, dot 1
- NMI is edge-triggered: `/NMI = !(nmi_output && nmi_occurred)`
- **Read timing**: CPU latches bus data at φ2 rising (~1 PPU tick into CPU cycle) = `target - 2` PPU ticks
- **Write timing**: CPU places write data at φ2 falling (~end of CPU cycle) = `target` PPU ticks
- **Suppression**: Reading $2002 on dot before VBlank → flag never set, no NMI. Same dot or +1 → flag reads set, NMI suppressed. +2 onward → normal.
- **Odd-frame skip**: At pre-render line dot 339, jump from (261,339) to (0,0), skipping idle tick at end of pre-render

## Files to Modify

1. `nes-ppu/src/ppu.rs` — PPU core: add suppress_vbl, fix update_nmi, fix odd-frame skip
2. `nes-ppu/src/bus_io.rs` — Catch-up bridge: read/write timing offsets, $2002 suppression logic

## Changes

### 1. `ppu.rs` — Add `suppress_vbl` field

Add `pub suppress_vbl: bool` to Ppu struct and initialize to `false` in `new()`.

### 2. `ppu.rs` — Fix `update_nmi()` falling-edge behavior

Change `update_nmi()` to accept a `from_tick: bool` parameter. Only cancel `nmi_pending` on falling edge when `from_tick == true` (i.e., during PPU tick processing). Register-write-triggered falling edges should NOT cancel pending NMI — once the CPU has had time to latch the NMI edge, the pending NMI is committed.

```rust
fn update_nmi(&mut self, from_tick: bool) {
    let active = self.nmi_output && self.nmi_occurred;
    if active && !self.nmi_line {
        self.nmi_pending = true;
    } else if !active && self.nmi_line {
        // Only tick-generated falling edges can cancel pending NMI.
        // Register writes happen too late in the CPU cycle to cancel
        // an already-committed NMI edge.
        if from_tick && self.nmi_write_delay == 0 {
            self.nmi_pending = false;
        }
    }
    self.nmi_line = active;
}
```

Update ALL call sites:
- `tick()`: pass `true`
- `read_register()` ($2002): pass `false`
- `write_register()` ($2000): pass `false`

### 3. `ppu.rs` — Fix `tick()` for VBlank suppression and odd-frame skip

```rust
pub fn tick(&mut self) {
    self.dot += 1;

    // Odd-frame skip: at end of pre-render line (261, dot 339),
    // skip directly to (0, 0) when rendering enabled on odd frames.
    // This replaces the idle tick at (261, 340).
    if self.scanline == 261 && self.dot == 340
        && self.odd_frame && self.rendering_enabled()
    {
        self.dot = 0;
        self.scanline = 0;
        self.odd_frame = false; // was odd → now even
        return;
    }

    if self.dot > 340 {
        self.dot = 0;
        self.scanline += 1;
        if self.scanline > 261 {
            self.scanline = 0;
            self.odd_frame = !self.odd_frame;
        }
    }

    // Pre-render scanline (261): clear flags at dot 1
    if self.scanline == 261 && self.dot == 1 {
        self.status &= !0xE0;
        self.nmi_occurred = false;
        self.update_nmi(true);
    }

    // Scanline 241 (start of VBlank): set flag at dot 1
    if self.scanline == 241 && self.dot == 1 {
        if self.suppress_vbl {
            // VBlank suppressed by $2002 read on exact cycle
            self.suppress_vbl = false;
        } else {
            self.status |= 0x80;
            self.nmi_occurred = true;
            self.update_nmi(true);
        }
    }
}
```

### 4. `ppu.rs` — NMI suppression window in `read_register` for $2002

After the existing $2002 read logic, add a check for the NMI suppression window. When reading $2002 within 0-2 PPU dots of VBlank set (scanline 241, dots 1-3), the NMI is suppressed because the /NMI line goes low then immediately back high, too fast for the CPU to latch:

```rust
PPU_2002_STATUS => {
    let effective_status = self.status;
    let val = (effective_status & 0xE0) | (self.data_bus & 0x1F);
    self.status &= !0x80;
    self.nmi_occurred = false;
    self.update_nmi(false);
    self.addr_latch = false;
    // NMI suppression window: reading $2002 within 2 PPU ticks of
    // VBlank set (241, dot 1-3) pulls /NMI high too quickly for CPU.
    if self.scanline == 241 && self.dot >= 1 && self.dot <= 3 {
        self.nmi_pending = false;
    }
    self.data_bus = val;
    val
}
```

### 5. `bus_io.rs` — Read/write catch-up timing offsets

Refactor catch_up into `catch_up_to(target)` and apply different offsets:

```rust
fn catch_up_to(&self, target_ppu: u64) {
    let current_ppu = self.ppu_cycles.get();
    if target_ppu > current_ppu {
        let ticks = target_ppu - current_ppu;
        let mut ppu = self.ppu.borrow_mut();
        for _ in 0..ticks {
            ppu.tick();
        }
        self.ppu_cycles.set(target_ppu);
    }
}
```

Read path — catch up to `target - 2` (reads happen early in CPU cycle):

```rust
fn read(&mut self, addr: u16) -> u8 {
    let target = self.cpu_cycles.get() * 3;
    let read_target = target.saturating_sub(2);
    self.catch_up_to(read_target);
    // VBlank suppression for $2002 reads
    if addr == 0x2002 {
        let mut ppu = self.ppu.borrow_mut();
        // If PPU is at (241, 0): next tick sets VBlank. Reading now
        // suppresses VBlank from ever being set this frame.
        if ppu.scanline == 241 && ppu.dot == 0 {
            ppu.suppress_vbl = true;
        }
        ppu.read_register(addr)
    } else {
        self.ppu.borrow_mut().read_register(addr)
    }
}
```

Write path — catch up to full `target` (writes happen late in CPU cycle):

```rust
fn write(&mut self, addr: u16, val: u8) {
    let target = self.cpu_cycles.get() * 3;
    self.catch_up_to(target);
    self.ppu.borrow_mut().write_register(addr, val);
}
```

Remove the dead `catch_up()` and `catch_up_and_read_status()` methods.

### 6. `ppu.rs` — Update unit tests

Update `test_vblank_timing` and `test_nmi_enable_during_vblank` to use `PPU_2002_STATUS` and `PPU_2000_CTRL` constants (already partially done by user).

## Test Expectations After Fix

| Test | Expected | Fix |
|------|----------|-----|
| 02 (vbl_set_time) | Row 04: `- -` suppression | VBlank suppression + read offset |
| 05 (nmi_timing) | Transitions at offsets 03, 09 | Read catch-up offset (-2 PPU ticks) |
| 06 (suppression) | Full suppression pattern | VBlank suppression + NMI suppression window |
| 07 (nmi_on_timing) | Stop at offset 04 | No falling-edge cancel from register writes |
| 08 (nmi_off_timing) | NMI fires at offset 07+ | No falling-edge cancel from register writes |
| 10 (even_odd_timing) | `08 08 09 07` | Odd-frame skip at pre-render end |

## Verification

```bash
cd d:/code/rust/rust-52-projects/nes-ppu
cargo test
cargo build --release
for t in 01 02 03 04 05 06 07 08 09 10; do
  cargo run --release -- roms/$t-*.nes 2>&1 | tail -5
done
```

All 10 tests should pass. Critically, verify tests 01, 03, 04, 09 still pass (no regressions).

## Risk: Read offset may need tuning

The -2 PPU tick read offset is based on hardware analysis (φ2 rising edge timing). If tests don't pass with -2, try -1 or -3 and observe which produces correct transition points for test 05 (expected transitions at rows 03 and 09).
