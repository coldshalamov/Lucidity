//! Translate WezTerm window events into egui input.

use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use ::window::{KeyCode, Modifiers as WezModifiers, MousePress};

pub fn egui_modifiers_from_wez(mods: WezModifiers) -> Modifiers {
    Modifiers {
        alt: mods.contains(WezModifiers::ALT),
        ctrl: mods.contains(WezModifiers::CTRL),
        shift: mods.contains(WezModifiers::SHIFT),
        mac_cmd: mods.contains(WezModifiers::SUPER) && cfg!(target_os = "macos"),
        command: if cfg!(target_os = "macos") {
            mods.contains(WezModifiers::SUPER)
        } else {
            mods.contains(WezModifiers::CTRL)
        },
    }
}

pub fn egui_pointer_button(button: MousePress) -> Option<PointerButton> {
    match button {
        MousePress::Left => Some(PointerButton::Primary),
        MousePress::Right => Some(PointerButton::Secondary),
        MousePress::Middle => Some(PointerButton::Middle),
    }
}

pub fn pointer_moved(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

pub fn pointer_button(
    pos: Pos2,
    button: PointerButton,
    pressed: bool,
    modifiers: Modifiers,
) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    }
}

pub fn egui_key_from_wez(key: &KeyCode) -> Option<Key> {
    match key {
        KeyCode::Char(c) => match *c {
            '\u{1b}' => Some(Key::Escape),
            '\r' | '\n' => Some(Key::Enter),
            '\t' => Some(Key::Tab),
            '\u{8}' => Some(Key::Backspace),
            '\u{7f}' => Some(Key::Delete),
            ' ' => Some(Key::Space),
            'a' | 'A' => Some(Key::A),
            'b' | 'B' => Some(Key::B),
            'c' | 'C' => Some(Key::C),
            'd' | 'D' => Some(Key::D),
            'e' | 'E' => Some(Key::E),
            'f' | 'F' => Some(Key::F),
            'g' | 'G' => Some(Key::G),
            'h' | 'H' => Some(Key::H),
            'i' | 'I' => Some(Key::I),
            'j' | 'J' => Some(Key::J),
            'k' | 'K' => Some(Key::K),
            'l' | 'L' => Some(Key::L),
            'm' | 'M' => Some(Key::M),
            'n' | 'N' => Some(Key::N),
            'o' | 'O' => Some(Key::O),
            'p' | 'P' => Some(Key::P),
            'q' | 'Q' => Some(Key::Q),
            'r' | 'R' => Some(Key::R),
            's' | 'S' => Some(Key::S),
            't' | 'T' => Some(Key::T),
            'u' | 'U' => Some(Key::U),
            'v' | 'V' => Some(Key::V),
            'w' | 'W' => Some(Key::W),
            'x' | 'X' => Some(Key::X),
            'y' | 'Y' => Some(Key::Y),
            'z' | 'Z' => Some(Key::Z),
            '0' => Some(Key::Num0),
            '1' => Some(Key::Num1),
            '2' => Some(Key::Num2),
            '3' => Some(Key::Num3),
            '4' => Some(Key::Num4),
            '5' => Some(Key::Num5),
            '6' => Some(Key::Num6),
            '7' => Some(Key::Num7),
            '8' => Some(Key::Num8),
            '9' => Some(Key::Num9),
            _ => None,
        },
        KeyCode::PageUp => Some(Key::PageUp),
        KeyCode::PageDown => Some(Key::PageDown),
        KeyCode::End => Some(Key::End),
        KeyCode::Home => Some(Key::Home),
        KeyCode::LeftArrow => Some(Key::ArrowLeft),
        KeyCode::RightArrow => Some(Key::ArrowRight),
        KeyCode::UpArrow => Some(Key::ArrowUp),
        KeyCode::DownArrow => Some(Key::ArrowDown),
        KeyCode::Function(n) => match n {
            1 => Some(Key::F1),
            2 => Some(Key::F2),
            3 => Some(Key::F3),
            4 => Some(Key::F4),
            5 => Some(Key::F5),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_escape_and_n() {
        assert_eq!(
            egui_key_from_wez(&KeyCode::Char('\u{1b}')),
            Some(Key::Escape)
        );
        assert_eq!(egui_key_from_wez(&KeyCode::Char('n')), Some(Key::N));
    }
}
