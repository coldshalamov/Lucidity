//! Translate WezTerm window events into egui input.

use ::window::{KeyCode, KeyEvent, Modifiers as WezModifiers, MousePress};
use egui::{Event, Key, Modifiers, PointerButton, Pos2};

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

/// Pure mapping from a WezTerm key event into egui events (Key and/or Text).
///
/// This is the shipped path used by [`super::super::TermWindow::feed_product_ui_key`].
pub fn egui_events_from_key_event(event: &KeyEvent) -> Vec<Event> {
    egui_events_from_key_parts(&event.key, event.modifiers, event.key_is_down)
}

/// Pure core of key→egui conversion (testable without a full KeyEvent).
pub fn egui_events_from_key_parts(
    key: &KeyCode,
    modifiers: WezModifiers,
    key_is_down: bool,
) -> Vec<Event> {
    let mods = egui_modifiers_from_wez(modifiers);
    let mut events = Vec::new();

    if let Some(egui_key) = egui_key_from_wez(key) {
        events.push(Event::Key {
            key: egui_key,
            physical_key: None,
            pressed: key_is_down,
            repeat: false,
            modifiers: mods,
        });
    }

    // Printable text only on press, and only when not a chord that should stay
    // with the terminal/GUI shortcuts (ctrl/alt/cmd).
    if key_is_down && !mods.ctrl && !mods.alt && !mods.command && !mods.mac_cmd {
        match key {
            KeyCode::Char(c) if !c.is_control() => {
                events.push(Event::Text(c.to_string()));
            }
            KeyCode::Composed(text) if !text.is_empty() => {
                events.push(Event::Text(text.clone()));
            }
            _ => {}
        }
    }

    events
}

/// Whether the product GUI should consume this key event after mapping.
///
/// Uses last-frame focus state (standard egui host pattern), plus Escape always
/// offered when the GUI may have a modal (caller still decides via wants_*).
pub fn should_consume_for_gui(wants_keyboard_input: bool, events: &[Event]) -> bool {
    if wants_keyboard_input {
        return !events.is_empty();
    }
    false
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

    #[test]
    fn key_event_maps_to_egui_key_and_text() {
        let events = egui_events_from_key_parts(
            &KeyCode::Char('a'),
            WezModifiers::NONE,
            true,
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                Event::Key {
                    key: Key::A,
                    pressed: true,
                    ..
                }
            )),
            "expected Key::A pressed, got {events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::Text(t) if t == "a")),
            "expected Text(a), got {events:?}"
        );
    }

    #[test]
    fn ctrl_n_is_key_without_text() {
        let events = egui_events_from_key_parts(
            &KeyCode::Char('n'),
            WezModifiers::CTRL,
            true,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Key {
                key: Key::N,
                pressed: true,
                modifiers,
                ..
            } if modifiers.command || modifiers.ctrl
        )));
        assert!(
            !events.iter().any(|e| matches!(e, Event::Text(_))),
            "ctrl chords must not emit Text: {events:?}"
        );
    }

    #[test]
    fn key_up_does_not_emit_text() {
        let events = egui_events_from_key_parts(
            &KeyCode::Char('x'),
            WezModifiers::NONE,
            false,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Key {
                key: Key::X,
                pressed: false,
                ..
            }
        )));
        assert!(!events.iter().any(|e| matches!(e, Event::Text(_))));
    }

    #[test]
    fn consume_when_gui_wants_keyboard() {
        let events = egui_events_from_key_parts(
            &KeyCode::Char('b'),
            WezModifiers::NONE,
            true,
        );
        assert!(should_consume_for_gui(true, &events));
        assert!(!should_consume_for_gui(false, &events));
        assert!(!should_consume_for_gui(true, &[]));
    }

    #[test]
    fn key_event_struct_path_matches_parts() {
        let event = KeyEvent {
            key: KeyCode::Char('z'),
            modifiers: WezModifiers::NONE,
            leds: Default::default(),
            repeat_count: 1,
            key_is_down: true,
            raw: None,
            #[cfg(windows)]
            win32_uni_char: None,
        };
        let from_event = egui_events_from_key_event(&event);
        let from_parts = egui_events_from_key_parts(&event.key, event.modifiers, event.key_is_down);
        assert_eq!(from_event.len(), from_parts.len());
        assert!(from_event.iter().any(|e| matches!(e, Event::Text(t) if t == "z")));
    }
}
