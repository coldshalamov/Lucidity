use mux::termwiztermtab::TermWizTerminal;
use termwiz::cell::AttributeChange;
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, Modifiers};
use termwiz::surface::{Change, CursorVisibility, Position};
use termwiz::terminal::Terminal;

fn build_pairing_screen() -> anyhow::Result<String> {
    let store_path = config::DATA_DIR.join("lucidity").join("host_keypair.json");
    let store = lucidity_pairing::KeypairStore::open(&store_path);
    let keypair = store.load_or_generate()?;

    let payload = lucidity_pairing::PairingPayload::new(keypair.public_key());
    let qr = lucidity_pairing::generate_pairing_qr_ascii(&payload)?;

    let mut s = String::new();
    s.push_str("Lucidity\r\n\r\n");
    s.push_str("Connect Lucidity Mobile\r\n\r\n");
    s.push_str(&qr);
    s.push_str("\r\n");
    s.push_str(&format!("Code: {}\r\n", payload.relay_id));
    s.push_str("\r\n");
    s.push_str("Scan in the mobile app or enter code.\r\n");
    s.push_str("Press Enter to continue locally.  (R = refresh)\r\n");
    s.push_str("\r\n");
    Ok(s)
}

fn build_pairing_screen_fallback(err: anyhow::Error) -> String {
    let mut s = String::new();
    s.push_str("Lucidity\r\n\r\n");
    s.push_str("Connect Lucidity Mobile\r\n\r\n");
    s.push_str("Error generating pairing QR.\r\n");
    s.push_str(&format!("{err:#}\r\n"));
    s.push_str("\r\nPress Enter to continue locally.\r\n");
    s
}

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

    let mut content =
        build_pairing_screen().unwrap_or_else(|err| build_pairing_screen_fallback(err));
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
                content =
                    build_pairing_screen().unwrap_or_else(|err| build_pairing_screen_fallback(err));
                render(&mut term, &content)?;
            }
            _ => {}
        }
    }

    Ok(())
}
