#![cfg(feature = "demos")]

use iced::event::{self, Event};
use iced::keyboard::{self, Key};
use iced::mouse;
use iced::time::Duration;
use iced::widget::canvas::{self, Cache, Canvas, Geometry};
use iced::widget::container;
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Size, Subscription, Task, Theme};
use nes_cpu::bus::Bus;
use nes_cpu::cpu::Cpu;
use nes_cpu::ines::{INesRom, Mirroring};
use rand::Rng;

const PIXEL_SIZE: f32 = 10.0;
const GRID_SIZE: usize = 32;
const SCREEN_START: u16 = 0x0200;
const SCREEN_LEN: usize = GRID_SIZE * GRID_SIZE;

const STEPS_PER_TICK: usize = 100;

#[rustfmt::skip]
const SNAKE_ROM: [u8; 309] = [
    0x20, 0x06, 0x06, 0x20, 0x38, 0x06, 0x20, 0x0d, 0x06, 0x20, 0x2a, 0x06, 0x60, 0xa9, 0x02, 0x85,
    0x02, 0xa9, 0x04, 0x85, 0x03, 0xa9, 0x11, 0x85, 0x10, 0xa9, 0x10, 0x85, 0x12, 0xa9, 0x0f, 0x85,
    0x14, 0xa9, 0x04, 0x85, 0x11, 0x85, 0x13, 0x85, 0x15, 0x60, 0xa5, 0xfe, 0x85, 0x00, 0xa5, 0xfe,
    0x29, 0x03, 0x18, 0x69, 0x02, 0x85, 0x01, 0x60, 0x20, 0x4d, 0x06, 0x20, 0x8d, 0x06, 0x20, 0xc3,
    0x06, 0x20, 0x19, 0x07, 0x20, 0x20, 0x07, 0x20, 0x2d, 0x07, 0x4c, 0x38, 0x06, 0xa5, 0xff, 0xc9,
    0x77, 0xf0, 0x0d, 0xc9, 0x64, 0xf0, 0x14, 0xc9, 0x73, 0xf0, 0x1b, 0xc9, 0x61, 0xf0, 0x22, 0x60,
    0xa9, 0x04, 0x24, 0x02, 0xd0, 0x26, 0xa9, 0x01, 0x85, 0x02, 0x60, 0xa9, 0x08, 0x24, 0x02, 0xd0,
    0x1b, 0xa9, 0x02, 0x85, 0x02, 0x60, 0xa9, 0x01, 0x24, 0x02, 0xd0, 0x10, 0xa9, 0x04, 0x85, 0x02,
    0x60, 0xa9, 0x02, 0x24, 0x02, 0xd0, 0x05, 0xa9, 0x08, 0x85, 0x02, 0x60, 0x60, 0x20, 0x94, 0x06,
    0x20, 0xa8, 0x06, 0x60, 0xa5, 0x00, 0xc5, 0x10, 0xd0, 0x0d, 0xa5, 0x01, 0xc5, 0x11, 0xd0, 0x07,
    0xe6, 0x03, 0xe6, 0x03, 0x20, 0x2a, 0x06, 0x60, 0xa2, 0x02, 0xb5, 0x10, 0xc5, 0x10, 0xd0, 0x06,
    0xb5, 0x11, 0xc5, 0x11, 0xf0, 0x09, 0xe8, 0xe8, 0xe4, 0x03, 0xf0, 0x06, 0x4c, 0xaa, 0x06, 0x4c,
    0x35, 0x07, 0x60, 0xa6, 0x03, 0xca, 0x8a, 0xb5, 0x10, 0x95, 0x12, 0xca, 0x10, 0xf9, 0xa5, 0x02,
    0x4a, 0xb0, 0x09, 0x4a, 0xb0, 0x19, 0x4a, 0xb0, 0x1f, 0x4a, 0xb0, 0x2f, 0xa5, 0x10, 0x38, 0xe9,
    0x20, 0x85, 0x10, 0x90, 0x01, 0x60, 0xc6, 0x11, 0xa9, 0x01, 0xc5, 0x11, 0xf0, 0x28, 0x60, 0xe6,
    0x10, 0xa9, 0x1f, 0x24, 0x10, 0xf0, 0x1f, 0x60, 0xa5, 0x10, 0x18, 0x69, 0x20, 0x85, 0x10, 0xb0,
    0x01, 0x60, 0xe6, 0x11, 0xa9, 0x06, 0xc5, 0x11, 0xf0, 0x0c, 0x60, 0xc6, 0x10, 0xa5, 0x10, 0x29,
    0x1f, 0xc9, 0x1f, 0xf0, 0x01, 0x60, 0x4c, 0x35, 0x07, 0xa0, 0x00, 0xa5, 0xfe, 0x91, 0x00, 0x60,
    0xa6, 0x03, 0xa9, 0x00, 0x81, 0x10, 0xa2, 0x00, 0xa9, 0x01, 0x81, 0x10, 0x60, 0xa2, 0x00, 0xea,
    0xea, 0xca, 0xd0, 0xfb, 0x60,
];

fn color_for(byte: u8) -> Color {
    match byte {
        0 => Color::BLACK,
        1 => Color::WHITE,
        2 | 9 => Color::from_rgb(0.6, 0.6, 0.6),
        3 | 10 => Color::from_rgb(1.0, 0.0, 0.0),
        4 | 11 => Color::from_rgb(0.0, 1.0, 0.0),
        5 | 12 => Color::from_rgb(0.0, 0.0, 1.0),
        6 | 13 => Color::from_rgb(1.0, 0.0, 1.0),
        7 | 14 => Color::from_rgb(1.0, 1.0, 0.0),
        _ => Color::from_rgb(0.0, 1.0, 1.0),
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .window_size(Size::new(
            GRID_SIZE as f32 * PIXEL_SIZE,
            GRID_SIZE as f32 * PIXEL_SIZE,
        ))
        .title("Snake — NES CPU")
        .run()
}

struct App {
    cpu: Cpu,
    screen: [u8; SCREEN_LEN],
    last_key: u8,
    cache: Cache,
    game_over: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    KeyPressed(u8),
    Ignore,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let rom = INesRom {
            prg_rom: vec![0u8; 0x4000],
            chr_rom: Vec::new(),
            mapper: 0,
            mirroring: Mirroring::Horizontal,
            has_battery: false,
        };
        let bus = Bus::from_rom(rom);
        let mut cpu = Cpu::new(bus);
        cpu.load_program(&SNAKE_ROM);

        (
            Self {
                cpu,
                screen: [0u8; SCREEN_LEN],
                last_key: 0,
                cache: Cache::new(),
                game_over: false,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::KeyPressed(key) => {
                self.last_key = key;
            }
            Message::Tick => {
                if self.game_over {
                    return Task::none();
                }

                let last_key = self.last_key;
                let mut rng = rand::thread_rng();

                self.cpu.bus.write(0xFF, last_key);
                self.cpu.bus.write(0xFE, rng.gen_range(1..16));

                for _ in 0..STEPS_PER_TICK {
                    self.cpu.step();

                    if self.cpu.bus.read(self.cpu.pc) == 0x00 {
                        self.game_over = true;
                        break;
                    }
                }

                let mut changed = false;
                for i in 0..SCREEN_LEN {
                    let val = self.cpu.bus.read(SCREEN_START + i as u16);
                    if self.screen[i] != val {
                        self.screen[i] = val;
                        changed = true;
                    }
                }
                if changed {
                    self.cache.clear();
                }
            }
            Message::Ignore => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        container(Canvas::new(self).width(Fill).height(Fill)).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let tick = if !self.game_over {
            iced::time::every(Duration::from_millis(15)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };

        let keys = event::listen().map(|event| match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => match key.as_ref() {
                Key::Character("w") => Message::KeyPressed(0x77),
                Key::Character("a") => Message::KeyPressed(0x61),
                Key::Character("s") => Message::KeyPressed(0x73),
                Key::Character("d") => Message::KeyPressed(0x64),
                _ => Message::Ignore,
            },
            _ => Message::Ignore,
        });

        Subscription::batch([tick, keys])
    }
}

impl canvas::Program<Message> for App {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), Color::BLACK);

            for i in 0..SCREEN_LEN {
                let byte = self.screen[i];
                if byte == 0 {
                    continue;
                }
                let x = (i % GRID_SIZE) as f32 * PIXEL_SIZE;
                let y = (i / GRID_SIZE) as f32 * PIXEL_SIZE;
                frame.fill_rectangle(
                    Point::new(x, y),
                    Size::new(PIXEL_SIZE, PIXEL_SIZE),
                    color_for(byte),
                );
            }
        });

        vec![geometry]
    }
}
