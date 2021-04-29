use core::{mem::MaybeUninit, ops::{Index, IndexMut}};

use x86_64::structures::idt::InterruptStackFrame;


pub unsafe fn initialize() {


    crate::idt::register_irq(0x20 + 1, interrupt_handler);
}

static mut DRIVER: Driver = Driver::new();

struct Driver {
    state: DriverState,
    pressed_keys: [bool; 256]
}

impl Driver {
    const fn new() -> Self {
        Self {
            state: DriverState::WaitingForNewKeypress,
            pressed_keys: [false; 256]
        }
    }

    fn handle_byte(&mut self, byte: u8) {
        match &mut self.state {
            DriverState::WaitingForNewKeypress => {
                // All bytes lower than 0xE0 are single byte scancodes
                if byte >= 0xE0 {
                    let mut svec = SVec::new();
                    svec.push(byte);
                    self.state = DriverState::InTheMiddleOfReceivingAKeypress(svec);
                } else {
                    self.handle_scancode(&mut [byte]);
                }
            }
            DriverState::InTheMiddleOfReceivingAKeypress(svec) => {
                fn handle_scancode_helper<const N: usize>(s: &mut Driver, mut svec: SVec<u8, N>, byte: u8) {
                    svec.push(byte);
                    s.handle_scancode(svec.get_slice_mut());
                    s.state = DriverState::WaitingForNewKeypress;
                }

                match (svec.get_slice(), byte) {
                    (&[0xE0], 0x2A | 0xB7) => svec.push(byte),
                    (&[0xE0, _], _) => svec.push(byte),
                    (&[0xE0, _, _], _) => {
                        let svec = svec.clone();
                        handle_scancode_helper(self, svec, byte);
                    },
                    (slice @ &[0xE1, ..], _) => {
                        let len = slice.len();
                        if len >= 5 {
                            let svec = svec.clone();
                            handle_scancode_helper(self, svec, byte);
                        } else {
                            svec.push(byte);
                        }
                    }
                    (&[0xE0], _) => {
                        let svec = svec.clone();
                        handle_scancode_helper(self, svec, byte);
                    },
                    _ => {
                        svec.push(byte);
                        panic!("Unrecognized byte sequence {:#X?}", svec.get_slice());
                    }
                }
            }
        }
    }

    fn handle_scancode(&mut self, scancode: &mut [u8]) {
        let was_released = match scancode {
            [b] | [0xE0, b] => {
                let was_released = *b & 0x80 != 0;
                *b &= !0x80;
                was_released
            },
            [0xE0, 0x2A, 0xE0, 0x37] => false,
            [0xE0, 0xB7, 0xE0, 0xAA] => {
                scancode[1] = 0x2A;
                scancode[3] = 0x37;
                true
            }
            _ => false
        };

        let keycode = match scancode {
            [0x01] => KeyCode::Escape,
            [0x02] => KeyCode::Digit1,
            [0x03] => KeyCode::Digit2,
            [0x04] => KeyCode::Digit3,
            [0x05] => KeyCode::Digit4,
            [0x06] => KeyCode::Digit5,
            [0x07] => KeyCode::Digit6,
            [0x08] => KeyCode::Digit7,
            [0x09] => KeyCode::Digit8,
            [0x0A] => KeyCode::Digit9,
            [0x0B] => KeyCode::Digit0,
            [0x0C] => KeyCode::Plus,
            [0x0D] => KeyCode::Accent,
            [0x0E] => KeyCode::Backspace,
            [0x0F] => KeyCode::Tab,
            [0x10] => KeyCode::Q,
            [0x11] => KeyCode::W,
            [0x12] => KeyCode::E,
            [0x13] => KeyCode::R,
            [0x14] => KeyCode::T,
            [0x15] => KeyCode::Y,
            [0x16] => KeyCode::U,
            [0x17] => KeyCode::I,
            [0x18] => KeyCode::O,
            [0x19] => KeyCode::P,
            [0x1A] => KeyCode::Å,
            [0x1B] => KeyCode::Umlaut,
            [0x1C] => KeyCode::Enter,
            [0x1E] => KeyCode::A,
            [0x1F] => KeyCode::S,
            [0x20] => KeyCode::D,
            [0x21] => KeyCode::F,
            [0x22] => KeyCode::G,
            [0x23] => KeyCode::H,
            [0x24] => KeyCode::J,
            [0x25] => KeyCode::K,
            [0x26] => KeyCode::L,
            [0x27] => KeyCode::Ö,
            [0x28] => KeyCode::Ä,
            [0x29] => KeyCode::Apostrophe,
            [0x2A] => KeyCode::LeftShift,
            [0x2B] => KeyCode::LessThan,
            [0x2C] => KeyCode::Z,
            [0x2D] => KeyCode::X,
            [0x2E] => KeyCode::C,
            [0x2F] => KeyCode::V,
        };
    }
}

enum DriverState {
    WaitingForNewKeypress,
    InTheMiddleOfReceivingAKeypress(SVec<u8, 6>)
}



pub struct KeyEvent {
    keycode: KeyCode,
    modifiers: Modifiers,
    char: Option<char>,
    state: KeyState
}

// TODO: Add explicit discriminant values
pub enum KeyCode {
    Unknown,

    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    PrintScreen,
    ScrollLock,
    PauseBreak,

    Paragraph,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Digit0,
    Plus,
    Accent,
    Backspace,
    Insert,
    Home,
    PageUp,
    NumLock,
    NumpadDivide,
    NumpadMultiply,
    NumbadSubtract,
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    Å,
    Umlaut,
    Enter,
    Delete,
    End,
    PageDown,
    Numpad7,
    Numpad8,
    Numpad9,
    NumbadAdd,

    CapsLock,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Ö,
    Ä,
    Apostrophe,
    Numpad4,
    Numpad5,
    Numpad6,

    LeftShift,
    LessThan,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Dash,
    RightShift,
    Up,
    Numpad1,
    Numpad2,
    Numpad3,
    NumpadEnter,

    LeftControl,
    LeftMeta,
    LeftAlt,
    Space,
    AltGr,
    RightMeta,
    Menu,
    RightControl,
    Left,
    Down,
    Right,
    Numpad0,
    NumpadDecimal,


    VolumeUp,
    VolumeDown,
    PlayPause,
    Calculator,
    PreviousTrack,
    NextTrack,
    Mute
}

pub enum KeyState {
    Pressed,
    Held
}

pub struct Modifiers {
    shift: bool,
    alt: bool,
    altgr: bool,
    ctrl: bool,
    meta: bool
}

pub fn get_key_event() -> KeyEvent {
    todo!()
}

extern "x86-interrupt" fn interrupt_handler(_: InterruptStackFrame) {
    let byte = unsafe { crate::ps2::get_byte() };
    // print!("{:02X} ", byte);
    unsafe {
        DRIVER.handle_byte(byte);
    }

    unsafe { crate::pic::send_eoi(1) };
}










struct SVec<T, const N: usize> {
    inner: [MaybeUninit<T>; N],
    length: usize
}

impl<T, const N: usize> SVec<T, N> {
    pub fn new() -> Self {
        Self {
            inner: MaybeUninit::uninit_array(),
            length: 0
        }
    }
}

impl<T, const N: usize> SVec<T, N> {
    pub fn len(&self) -> usize {
        self.length
    }

    pub fn capacity(&self) -> usize {
        N
    }

    pub fn push(&mut self, value: T) {
        self.inner[self.length] = MaybeUninit::new(value);
        self.length += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.length > 0 {
            self.length -= 1;
            Some(unsafe { self.inner[self.length].assume_init_read() })
        } else {
            None
        }
    }

    pub fn get_slice(&self) -> &[T] {
        unsafe {
            core::mem::transmute(&self.inner[..self.length])
        }
    }

    pub fn get_slice_mut(&mut self) -> &mut [T] {
        unsafe {
            core::mem::transmute(&mut self.inner[..self.length])
        }
    }
}

impl<T, const N: usize> Index<usize> for SVec<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.length {
            panic!("Index out of bounds; index was {}, max was {}", index, self.length - 1);
        } else {
            unsafe { self.inner[index].assume_init_ref() }
        }
    }
}

impl<T, const N: usize> IndexMut<usize> for SVec<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.length {
            panic!("Index out of bounds; index was {}, max was {}", index, self.length - 1);
        } else {
            unsafe { self.inner[index].assume_init_mut() }
        }
    }
}

impl<T: Clone, const N: usize> Clone for SVec<T, N> {
    fn clone(&self) -> Self {
        let mut ret = SVec::new();
        for i in self.get_slice() {
            ret.push(i.clone());
        }
        ret
    }
}

impl<T, const N: usize> Drop for SVec<T, N> {
    fn drop(&mut self) {
        for item in self.get_slice_mut() {
            core::mem::drop(item);
        }
    }
}
