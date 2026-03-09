use std::cell::{Cell, RefCell};
use std::rc::Rc;

use nes_cpu::bus::Bus;
use nes_cpu::cpu::Cpu;
use nes_cpu::ines::INesRom;
use nes_cpu::mapper::Mapper;
use nes_joypad::composite_io::CompositeBusIo;
use nes_joypad::joypad::Joypad;
use nes_ppu::bus_io::PpuBusIo;
use nes_ppu::ppu::Ppu;

use crate::apu::Apu;
use crate::composite_io::ApuCompositeBusIo;

/// Integrated NES system with CPU, PPU, APU, and joypad support.
pub struct System {
    pub cpu: Cpu,
    pub ppu: Rc<RefCell<Ppu>>,
    pub apu: Rc<RefCell<Apu>>,
    pub joypad1: Rc<RefCell<Joypad>>,
    pub joypad2: Rc<RefCell<Joypad>>,
    cpu_cycles: Rc<Cell<u64>>,
    ppu_cycles: Rc<Cell<u64>>,
}

impl System {
    pub fn from_rom(rom: INesRom, sample_rate: f64) -> anyhow::Result<Self> {
        let mapper: Rc<RefCell<Box<dyn Mapper>>> =
            Rc::new(RefCell::new(nes_mapper::from_rom(&rom)?));

        let mut bus = Bus::new();
        bus.mapper = Some(Rc::clone(&mapper));
        let ppu = Rc::new(RefCell::new(Ppu::new(Rc::clone(&mapper))));
        let apu = Rc::new(RefCell::new(Apu::new(sample_rate)));
        let cpu_cycles = Rc::new(Cell::new(0u64));
        let ppu_cycles = Rc::new(Cell::new(0u64));
        let joypad1 = Rc::new(RefCell::new(Joypad::new()));
        let joypad2 = Rc::new(RefCell::new(Joypad::new()));

        bus.io = Some(Box::new(ApuCompositeBusIo {
            inner: CompositeBusIo {
                ppu_io: PpuBusIo {
                    ppu: Rc::clone(&ppu),
                    cpu_cycles: Rc::clone(&cpu_cycles),
                    ppu_cycles: Rc::clone(&ppu_cycles),
                },
                joypad1: Rc::clone(&joypad1),
                joypad2: Rc::clone(&joypad2),
            },
            apu: Rc::clone(&apu),
        }));

        let mut cpu = Cpu::new(bus);
        cpu.cycles = 0;
        cpu.reset();

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

        // Sync APU to match CPU reset cycles
        {
            let mut a = apu.borrow_mut();
            for _ in 0..cpu.cycles {
                a.tick();
            }
            // Discard startup samples
            a.drain_samples();
        }

        Ok(Self {
            cpu,
            ppu,
            apu,
            joypad1,
            joypad2,
            cpu_cycles,
            ppu_cycles,
        })
    }

    /// Step one CPU instruction, ticking PPU and APU.
    pub fn step(&mut self) -> u64 {
        // Service pending NMI from PPU
        if self.ppu.borrow_mut().take_nmi() {
            self.cpu.nmi();
            self.cpu_cycles.set(self.cpu.cycles);
        }

        // Service pending IRQ from APU
        if self.apu.borrow().irq_pending() {
            self.cpu.irq();
            self.cpu_cycles.set(self.cpu.cycles);
        }

        // Handle OAM DMA if requested
        let dma_cycles = self.handle_dma();

        // Pre-read instruction for PPU catch-up timing
        let opcode = self.cpu.bus.peek(self.cpu.pc);
        let instr = nes_cpu::opcodes::get_opcodes()[opcode as usize];
        let base_cost = instr.cycles as u64;
        self.cpu_cycles.set(self.cpu.cycles + base_cost);

        let prev_cycles = self.cpu.cycles;
        let _ = self.cpu.step();
        let elapsed = self.cpu.cycles - prev_cycles;

        // Update shared counter with actual final cycle count
        self.cpu_cycles.set(self.cpu.cycles);

        // Tick PPU for remaining cycles not caught up during register access
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

        // Tick APU for the elapsed CPU cycles
        {
            let mut apu = self.apu.borrow_mut();
            for _ in 0..total_cpu_cycles {
                apu.tick();
            }

            // Handle DMC sample fetch request
            if let Some(addr) = apu.dmc_sample_request() {
                drop(apu);
                let val = self.cpu.bus.read(addr);
                let mut apu = self.apu.borrow_mut();
                apu.dmc_load_sample(val);
                // DMC fetch steals 4 CPU cycles
                self.cpu.cycles += 4;
                self.cpu_cycles.set(self.cpu.cycles);
            }
        }

        total_cpu_cycles
    }

    /// Run CPU/PPU/APU until the PPU completes a frame (VBlank begins).
    pub fn run_until_frame(&mut self) {
        loop {
            self.step();
            if self.ppu.borrow().frame_ready {
                self.ppu.borrow_mut().frame_ready = false;
                return;
            }
        }
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

        let dma_cost = 513u64;
        self.cpu.cycles += dma_cost;
        self.cpu_cycles.set(self.cpu.cycles);
        dma_cost
    }
}
