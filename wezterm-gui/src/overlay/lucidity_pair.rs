use mux::termwiztermtab::TermWizTerminal;
use termwiz::cell::AttributeChange;
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, Modifiers};
use termwiz::surface::{Change, CursorVisibility, Position};
use termwiz::terminal::Terminal;

fn render(term: &mut TermWizTerminal, content: &str) -> termwiz::Result<()> {
    let mut changes = vec![
        Change::ClearScreen(ColorAttribute::Default),
        Change::CursorVisibility(CursorVisibility::Hidden),
        AttributeChange::Intensity(termwiz::cell::Intensity::Bold).into(),
        Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(0),
        },
    ];

    let size = term.get_screen_size()?;
    let max_rows = size.rows.saturating_sub(1);

    for (row, line) in content.split("\r\n").enumerate() {
        if row >= max_rows {
            break;
        }
        changes.push(Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(row),
        });
        changes.push(Change::Text(line.to_string()));
    }

    term.render(&changes)?;
    term.flush()
}

pub fn lucidity_pair_overlay(mut term: TermWizTerminal) -> anyhow::Result<()> {
    term.set_raw_mode()?;
    term.no_grab_mouse_in_raw_mode();

    let mut content = lucidity_host::pairing_display_text();
    render(&mut term, &content)?;

    while let Ok(Some(event)) = term.poll_input(None) {
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Enter,
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Char('g' | 'G'),
                modifiers: Modifiers::CTRL,
            }) => {
                break;
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('r' | 'R'),
                ..
            }) => {
                lucidity_host::pairing_rotate();
                content = lucidity_host::pairing_display_text();
                render(&mut term, &content)?;
            }
            _ => {}
        }
    }

    Ok(())
}

