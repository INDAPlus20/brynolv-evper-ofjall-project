
pub fn initialize() {
    todo!()
}

pub struct KeyEvent {
    keycode: u8,
    modifiers: Modifiers,
    char: Option<char>,
    state: KeyState
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
