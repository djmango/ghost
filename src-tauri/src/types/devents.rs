use rdev::{Button, Key};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] // JSON value name
pub enum MouseAction {
    Left,
    Right,
    Middle,
    Other(u8)
}

impl fmt::Display for MouseAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MouseAction::Left => write!(f, "left"),
            MouseAction::Right => write!(f, "right"),
            MouseAction::Middle => write!(f, "middle"),
            MouseAction::Other(_) => write!(f, "other")
        }
    }
}

impl Into<MouseAction> for Button {
    fn into(self) -> MouseAction {
        match self {
            Button::Left => MouseAction::Left,
            Button::Right => MouseAction::Right,
            Button::Middle => MouseAction::Middle,
            Button::Unknown(byte) => MouseAction::Other(byte),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] // JSON value name
pub enum KeyboardActionKey {
    // Modifier Keys
    #[serde(rename = "caps_lock")]
    CapsLock,
    Shift,
    Control,
    Fn,
    Alt,
    Meta,
    // Function Keys
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
    // Alphabet Keys
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    // Number Keys
    #[serde(rename = "0")]
    Num0,
    #[serde(rename = "1")]
    Num1,
    #[serde(rename = "2")]
    Num2,
    #[serde(rename = "3")]
    Num3,
    #[serde(rename = "4")]
    Num4,
    #[serde(rename = "5")]
    Num5,
    #[serde(rename = "6")]
    Num6,
    #[serde(rename = "7")]
    Num7,
    #[serde(rename = "8")]
    Num8,
    #[serde(rename = "9")]
    Num9,
    // Navigation Keys
    #[serde(rename = "arrow_up")]
    ArrowUp,
    #[serde(rename = "arrow_down")]
    ArrowDown,
    #[serde(rename = "arrow_left")]
    ArrowLeft,
    #[serde(rename = "arrow_right")]
    ArrowRight,
    Home,
    End,
    #[serde(rename = "page_up")]
    PageUp,
    #[serde(rename = "page_down")]
    PageDown,
    // Special Keys
    Escape,
    Enter,
    Tab,
    Space,
    Backspace,
    Insert,
    Delete,
    #[serde(rename = "num_lock")]
    NumLock,
    #[serde(rename = "scroll_lock")]
    ScrollLock,
    Pause,
    #[serde(rename = "print_screen")]
    PrintScreen,
    // Symbols
    Grave,
    Minus,
    Equal,
    #[serde(rename = "bracket_left")]
    BracketLeft,
    #[serde(rename = "bracket_right")]
    BracketRight,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Backslash,
    Unknown(u32),
    // RawKey(RawKey),
}

impl Into<KeyboardActionKey> for rdev::Key {
    fn into(self) -> KeyboardActionKey {
        match self {
            Key::Alt => KeyboardActionKey::Alt,
            Key::AltGr => KeyboardActionKey::Alt,
            Key::Backspace => KeyboardActionKey::Backspace,
            Key::CapsLock => KeyboardActionKey::CapsLock,
            Key::ControlLeft => KeyboardActionKey::Control,
            Key::ControlRight => KeyboardActionKey::Control,
            Key::Delete => KeyboardActionKey::Delete,
            Key::DownArrow => KeyboardActionKey::ArrowDown,
            Key::End => KeyboardActionKey::End,
            Key::Escape => KeyboardActionKey::Escape,
            Key::F1 => KeyboardActionKey::F1,
            Key::F10 => KeyboardActionKey::F10,
            Key::F11 => KeyboardActionKey::F11,
            Key::F12 => KeyboardActionKey::F12,
            // Key::F13 => KeyboardActionKey::F13,
            // Key::F14 => KeyboardActionKey::F14,
            // Key::F15 => KeyboardActionKey::F15,
            // Key::F16 => KeyboardActionKey::F16,
            // Key::F17 => KeyboardActionKey::F17,
            // Key::F18 => KeyboardActionKey::F18,
            // Key::F19 => KeyboardActionKey::F19,
            // Key::F20 => KeyboardActionKey::F20,
            // Key::F21 => KeyboardActionKey::F21,
            // Key::F22 => KeyboardActionKey::F22,
            // Key::F23 => KeyboardActionKey::F23,
            // Key::F24 => KeyboardActionKey::F24,
            Key::F2 => KeyboardActionKey::F2,
            Key::F3 => KeyboardActionKey::F3,
            Key::F4 => KeyboardActionKey::F4,
            Key::F5 => KeyboardActionKey::F5,
            Key::F6 => KeyboardActionKey::F6,
            Key::F7 => KeyboardActionKey::F7,
            Key::F8 => KeyboardActionKey::F8,
            Key::F9 => KeyboardActionKey::F9,
            Key::Home => KeyboardActionKey::Home,
            Key::LeftArrow => KeyboardActionKey::ArrowLeft,
            Key::MetaLeft => KeyboardActionKey::Meta, // also known as "windows", "super", and "command"
            Key::MetaRight => KeyboardActionKey::Meta, // also known as "windows", "super", and "command"
            Key::PageDown => KeyboardActionKey::PageDown,
            Key::PageUp => KeyboardActionKey::PageUp,
            Key::Return => KeyboardActionKey::Enter,
            Key::RightArrow => KeyboardActionKey::ArrowRight,
            Key::ShiftLeft => KeyboardActionKey::Shift,
            Key::ShiftRight => KeyboardActionKey::Shift,
            Key::Space => KeyboardActionKey::Space,
            Key::Tab => KeyboardActionKey::Tab,
            Key::UpArrow => KeyboardActionKey::ArrowUp,
            Key::PrintScreen => KeyboardActionKey::PrintScreen,
            Key::ScrollLock => KeyboardActionKey::ScrollLock,
            Key::Pause => KeyboardActionKey::Pause,
            Key::NumLock => KeyboardActionKey::NumLock,
            Key::BackQuote => KeyboardActionKey::Grave,
            Key::Num1 => KeyboardActionKey::Num1,
            Key::Num2 => KeyboardActionKey::Num2,
            Key::Num3 => KeyboardActionKey::Num3,
            Key::Num4 => KeyboardActionKey::Num4,
            Key::Num5 => KeyboardActionKey::Num5,
            Key::Num6 => KeyboardActionKey::Num6,
            Key::Num7 => KeyboardActionKey::Num7,
            Key::Num8 => KeyboardActionKey::Num8,
            Key::Num9 => KeyboardActionKey::Num9,
            Key::Num0 => KeyboardActionKey::Num0,
            Key::Minus => KeyboardActionKey::Minus,
            Key::Equal => KeyboardActionKey::Equal,
            Key::KeyQ => KeyboardActionKey::Q,
            Key::KeyW => KeyboardActionKey::W,
            Key::KeyE => KeyboardActionKey::E,
            Key::KeyR => KeyboardActionKey::R,
            Key::KeyT => KeyboardActionKey::T,
            Key::KeyY => KeyboardActionKey::Y,
            Key::KeyU => KeyboardActionKey::U,
            Key::KeyI => KeyboardActionKey::I,
            Key::KeyO => KeyboardActionKey::O,
            Key::KeyP => KeyboardActionKey::P,
            Key::LeftBracket => KeyboardActionKey::BracketLeft,
            Key::RightBracket => KeyboardActionKey::BracketRight,
            Key::KeyA => KeyboardActionKey::A,
            Key::KeyS => KeyboardActionKey::S,
            Key::KeyD => KeyboardActionKey::D,
            Key::KeyF => KeyboardActionKey::F,
            Key::KeyG => KeyboardActionKey::G,
            Key::KeyH => KeyboardActionKey::H,
            Key::KeyJ => KeyboardActionKey::J,
            Key::KeyK => KeyboardActionKey::K,
            Key::KeyL => KeyboardActionKey::L,
            Key::SemiColon => KeyboardActionKey::Semicolon,
            Key::Quote => KeyboardActionKey::Quote,
            Key::BackSlash => KeyboardActionKey::Backslash,
            // Key::IntlBackslash => KeyboardActionKey::IntlBackslash,
            // Key::IntlRo => KeyboardActionKey::IntlRo,   // Brazilian /? and Japanese _ 'ro'
            // Key::IntlYen => KeyboardActionKey::IntlYen,  // Japanese Henkan (Convert) key.
            // Key::KanaMode => KeyboardActionKey::KanaMode, // Japanese Hiragana/Katakana key.
            Key::KeyZ => KeyboardActionKey::Z,
            Key::KeyX => KeyboardActionKey::X,
            Key::KeyC => KeyboardActionKey::C,
            Key::KeyV => KeyboardActionKey::V,
            Key::KeyB => KeyboardActionKey::B,
            Key::KeyN => KeyboardActionKey::N,
            Key::KeyM => KeyboardActionKey::M,
            Key::Comma => KeyboardActionKey::Comma,
            Key::Dot => KeyboardActionKey::Period,
            Key::Slash => KeyboardActionKey::Slash,
            Key::Insert => KeyboardActionKey::Insert,
            // Key::KpReturn => KeyboardActionKey::KpReturn,
            // Key::KpMinus => KeyboardActionKey::KpMinus,
            // Key::KpPlus => KeyboardActionKey::KpPlus,
            // Key::KpMultiply => KeyboardActionKey::KpMultiply,
            // Key::KpDivide => KeyboardActionKey::KpDivide,
            // Key::KpDecimal => KeyboardActionKey::KpDecimal,
            // Key::KpEqual => KeyboardActionKey::KpEqual,
            // Key::KpComma => KeyboardActionKey::KpComma,
            // Key::Kp0 => KeyboardActionKey::Kp0,
            // Key::Kp1 => KeyboardActionKey::Kp1,
            // Key::Kp2 => KeyboardActionKey::Kp2,
            // Key::Kp3 => KeyboardActionKey::Kp3,
            // Key::Kp4 => KeyboardActionKey::Kp4,
            // Key::Kp5 => KeyboardActionKey::Kp5,
            // Key::Kp6 => KeyboardActionKey::Kp6,
            // Key::Kp7 => KeyboardActionKey::Kp7,
            // Key::Kp8 => KeyboardActionKey::Kp8,
            // Key::Kp9 => KeyboardActionKey::Kp9,
            // Key::VolumeUp => KeyboardActionKey::VolumeUp,
            // Key::VolumeDown => KeyboardActionKey::VolumeDown,
            // Key::VolumeMute => KeyboardActionKey::VolumeMute,
            // Key::Lang1 => KeyboardActionKey::Lang1, // Korean Hangul/English toggle key, and as the Kana key on the Apple Japanese keyboard.
            // Key::Lang2 => KeyboardActionKey::Lang2, // Korean Hanja conversion key, and as the Eisu key on the Apple Japanese keyboard.
            // Key::Lang3 => KeyboardActionKey::Lang3, // Japanese Katakana key.
            // Key::Lang4 => KeyboardActionKey::Lang4, // Japanese Hiragana key.
            // Key::Lang5 => KeyboardActionKey::Lang5, // Japanese Zenkaku/Hankaku (Fullwidth/halfwidth) key.
            Key::Function => KeyboardActionKey::Fn,
            // Key::Apps => KeyboardActionKey::Apps,
            // Key::Cancel => KeyboardActionKey::Cancel,
            // Key::Clear => KeyboardActionKey::Clear,
            // Key::Kana => KeyboardActionKey::Kana,
            // Key::Hangul => KeyboardActionKey::Hangul,
            // Key::Junja => KeyboardActionKey::Junja,
            // Key::Final => KeyboardActionKey::Final,
            // Key::Hanja => KeyboardActionKey::Hanja,
            // Key::Print => KeyboardActionKey::Print,
            // Key::Select => KeyboardActionKey::Select,
            // Key::Execute => KeyboardActionKey::Execute,
            // Key::Help => KeyboardActionKey::Help,
            // Key::Sleep => KeyboardActionKey::Sleep,
            // Key::Separator => KeyboardActionKey::Separator,
            Key::Unknown(u32) => KeyboardActionKey::Unknown(u32),
            // Key::RawKey(RawKey) => KeyboardActionKey::RawKey(RawKey),
            _ => KeyboardActionKey::Unknown(0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] // JSON value name
pub struct KeyboardAction {
    pub key: KeyboardActionKey,
    pub duration: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] // JSON value name
pub struct ScrollAction {
    pub x: i32,
    pub y: i32,
}
