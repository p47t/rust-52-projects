use std::cell::{Cell, RefCell};
use std::rc::Rc;

use nes_cpu::bus::Bus;
use nes_cpu::cpu::Cpu;
use nes_cpu::ines::INesRom;

use crate::bus_io::PpuBusIo;
use crate::ppu::Ppu;

pub struct System {
    pub cpu: Cpu,
    pub ppu: Rc<RefCell<Ppu>>,
    /// Shared CPU cycle counter — BusIo reads this to catch up the PPU.
    cpu_cycles: Rc<Cell<u64>>,
    /// PPU cycles already ticked — shared with BusIo to track catch-up.
    ppu_cycles: Rc<Cell<u64>>,
}

impl System {
    pub fn from_rom(rom: INesRom) -> Self {
        let chr_rom = rom.chr_rom.clone();
        let mirroring = rom.mirroring;

        let mut bus = Bus::from_rom(rom);
        let ppu = Rc::new(RefCell::new(Ppu::new(chr_rom, mirroring)));
        let cpu_cycles = Rc::new(Cell::new(0u64));
        let ppu_cycles = Rc::new(Cell::new(0u64));

        // Wire PPU registers into the CPU bus
        bus.io = Some(Box::new(PpuBusIo {
            ppu: Rc::clone(&ppu),
            cpu_cycles: Rc::clone(&cpu_cycles),
            ppu_cycles: Rc::clone(&ppu_cycles),
        }));

        let mut cpu = Cpu::new(bus);
        // Cpu::new sets cycles=7 for nestest compat; reset properly for PPU sync
        cpu.cycles = 0;
        cpu.reset(); // reads reset vector, sets I, SP-=3, +7 cycles

        // Sync PPU to match CPU reset cycles
        cpu_cycles.set(cpu.cycles);
        let reset_ppu_dots = cpu.cycles * 3;
        {
            let mut p = ppu.borrow_mut();
            for _ in 0..reset_ppu_dots {
                p.tick();
            }
        }
        ppu_cycles.set(reset_ppu_dots);

        Self { cpu, ppu, cpu_cycles, ppu_cycles }
    }

    /// Step one CPU instruction, ticking PPU via catch-up.
    /// Returns the number of CPU cycles elapsed.
    pub fn step(&mut self) -> u64 {
        // Service pending NMI
        if self.ppu.borrow_mut().take_nmi() {
            self.cpu.nmi();
            // Sync cycle counter after NMI push (7 cycles)
            self.cpu_cycles.set(self.cpu.cycles);
        }

        // Handle OAM DMA if requested
        let dma_cycles = self.handle_dma();

        // Pre-read instruction to determine base cycle cost, then set the
        // shared counter so PPU catch-up during bus reads sees correct timing.
        // The -1 accounts for the memory read happening at the start of the
        // last bus cycle (before the PPU ticks for that cycle complete).
        let opcode = self.cpu.bus.peek(self.cpu.pc);
        let instr = nes_cpu::opcodes::get_opcodes()[opcode as usize];
        let base_cost = instr.cycles as u64;
        self.cpu_cycles.set(self.cpu.cycles + base_cost);

        let prev_cycles = self.cpu.cycles;
        let _ = self.cpu.step();
        let elapsed = self.cpu.cycles - prev_cycles;

        // Update shared counter with actual final cycle count
        // (may differ from base due to page-cross penalties, branches)
        self.cpu_cycles.set(self.cpu.cycles);

        // Tick PPU for any remaining cycles not yet caught up
        // (happens when instructions don't touch PPU registers)
        let total_cpu_cycles = elapsed + dma_cycles;
        let target_ppu = self.cpu.cycles * 3;
        let current_ppu = self.ppu_cycles.get();
        if target_ppu > current_ppu {
            let remaining = target_ppu - current_ppu;
            let mut ppu = self.ppu.borrow_mut();
            for _ in 0..remaining {
                ppu.tick();
            }
            self.ppu_cycles.set(target_ppu);
        }

        total_cpu_cycles
    }

    fn handle_dma(&mut self) -> u64 {
        let page = {
            let mut ppu = self.ppu.borrow_mut();
            match ppu.dma_page.take() {
                Some(p) => p,
                None => return 0,
            }
        };

        let base = (page as u16) << 8;
        for i in 0..256u16 {
            let val = self.cpu.bus.read(base + i);
            let mut ppu = self.ppu.borrow_mut();
            let addr = ppu.oam_addr as usize;
            ppu.oam[addr] = val;
            ppu.oam_addr = ppu.oam_addr.wrapping_add(1);
        }

        // DMA takes 513 cycles (514 on odd CPU cycle, but we approximate)
        let dma_cost = 513u64;
        self.cpu.cycles += dma_cost;
        self.cpu_cycles.set(self.cpu.cycles);
        dma_cost
    }
}
