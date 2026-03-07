/// NES button indices matching the shift register read order.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Button {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
}

/// Emulates a single NES controller using the standard serial protocol.
///
/// The NES reads controllers through a strobe/shift-register mechanism:
/// 1. Write 1 then 0 to $4016 (strobe on, then off) to latch button state
/// 2. Read $4016/$4017 eight times to get each button (A, B, Select, Start, Up, Down, Left, Right)
/// 3. Further reads return 1 (open bus behavior for standard controllers)
pub struct Joypad {
    /// Live button state bitfield (bit N = Button N pressed).
    button_state: u8,
    /// Latched snapshot frozen on strobe falling edge.
    shift_register: u8,
    /// Next bit index to return on read (0–7, then open bus).
    read_index: u8,
    /// When true, shift register continuously reloads from button_state.
    strobe: bool,
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            button_state: 0,
            shift_register: 0,
            read_index: 0,
            strobe: false,
        }
    }

    /// Update the live button state. Called each frame by the input layer.
    pub fn set_buttons(&mut self, state: u8) {
        self.button_state = state;
        if self.strobe {
            self.shift_register = self.button_state;
        }
    }

    /// Handle $4016 write (strobe control).
    /// Bit 0 = 1: strobe on (continuously latch).
    /// Bit 0 = 0: strobe off (freeze shift register, reset read index).
    pub fn write_strobe(&mut self, val: u8) {
        let new_strobe = val & 1 != 0;
        if self.strobe && !new_strobe {
            // Falling edge: latch and reset
            self.shift_register = self.button_state;
            self.read_index = 0;
        }
        self.strobe = new_strobe;
        if self.strobe {
            self.shift_register = self.button_state;
        }
    }

    /// Handle $4016/$4017 read. Returns one button bit per call.
    /// While strobe is high, continuously returns the A button state.
    /// After 8 reads, returns 1 (open bus for standard controller).
    pub fn read_bit(&mut self) -> u8 {
        if self.strobe {
            return self.button_state & 1;
        }
        if self.read_index < 8 {
            let bit = (self.shift_register >> self.read_index) & 1;
            self.read_index += 1;
            bit
        } else {
            1 // open bus
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strobe_latches_all_buttons() {
        let mut jp = Joypad::new();
        jp.set_buttons(0xFF);
        jp.write_strobe(1);
        jp.write_strobe(0);

        for i in 0..8 {
            assert_eq!(jp.read_bit(), 1, "button {i} should be pressed");
        }
    }

    #[test]
    fn read_order_matches_button_enum() {
        let mut jp = Joypad::new();
        jp.set_buttons(1 << Button::Start as u8);
        jp.write_strobe(1);
        jp.write_strobe(0);

        assert_eq!(jp.read_bit(), 0); // A
        assert_eq!(jp.read_bit(), 0); // B
        assert_eq!(jp.read_bit(), 0); // Select
        assert_eq!(jp.read_bit(), 1); // Start
        assert_eq!(jp.read_bit(), 0); // Up
        assert_eq!(jp.read_bit(), 0); // Down
        assert_eq!(jp.read_bit(), 0); // Left
        assert_eq!(jp.read_bit(), 0); // Right
    }

    #[test]
    fn open_bus_after_8_reads() {
        let mut jp = Joypad::new();
        jp.write_strobe(1);
        jp.write_strobe(0);
        for _ in 0..8 {
            jp.read_bit();
        }
        assert_eq!(jp.read_bit(), 1);
        assert_eq!(jp.read_bit(), 1);
    }

    #[test]
    fn strobe_high_returns_a_button() {
        let mut jp = Joypad::new();
        jp.set_buttons(1 << Button::A as u8);
        jp.write_strobe(1);
        assert_eq!(jp.read_bit(), 1);
        assert_eq!(jp.read_bit(), 1); // still A, no advancing
    }

    #[test]
    fn strobe_high_no_a_returns_zero() {
        let mut jp = Joypad::new();
        jp.set_buttons(1 << Button::Start as u8); // A not pressed
        jp.write_strobe(1);
        assert_eq!(jp.read_bit(), 0);
    }

    #[test]
    fn no_buttons_reads_all_zero() {
        let mut jp = Joypad::new();
        jp.write_strobe(1);
        jp.write_strobe(0);
        for i in 0..8 {
            assert_eq!(jp.read_bit(), 0, "button {i} should not be pressed");
        }
    }

    #[test]
    fn multiple_buttons_simultaneously() {
        let mut jp = Joypad::new();
        jp.set_buttons((1 << Button::A as u8) | (1 << Button::Right as u8));
        jp.write_strobe(1);
        jp.write_strobe(0);

        assert_eq!(jp.read_bit(), 1); // A
        assert_eq!(jp.read_bit(), 0); // B
        assert_eq!(jp.read_bit(), 0); // Select
        assert_eq!(jp.read_bit(), 0); // Start
        assert_eq!(jp.read_bit(), 0); // Up
        assert_eq!(jp.read_bit(), 0); // Down
        assert_eq!(jp.read_bit(), 0); // Left
        assert_eq!(jp.read_bit(), 1); // Right
    }
}
