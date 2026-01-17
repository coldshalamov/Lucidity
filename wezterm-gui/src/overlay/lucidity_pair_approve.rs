use mux::termwiztermtab::TermWizTerminal;
use termwiz::cell::AttributeChange;
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, MouseButtons, MouseEvent};
use termwiz::surface::{Change, CursorVisibility, Position};
use termwiz::terminal::Terminal;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ActiveButton {
    None,
    Approve,
    Reject,
}

fn render(
    term: &mut TermWizTerminal,
    lines: &[String],
    x_pos: usize,
    top_row: usize,
    button_row: usize,
    active: ActiveButton,
    approve_x: usize,
    approve_w: usize,
    reject_x: usize,
    reject_w: usize,
) -> termwiz::Result<()> {
    let mut changes = vec![
        Change::ClearScreen(ColorAttribute::Default),
        Change::CursorVisibility(CursorVisibility::Hidden),
        Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(0),
        },
    ];

    for (y, line) in lines.iter().enumerate() {
        changes.push(Change::CursorPosition {
            x: Position::Absolute(x_pos),
            y: Position::Absolute(top_row + y),
        });

        if y == 0 {
            changes.push(AttributeChange::Intensity(termwiz::cell::Intensity::Bold).into());
        }
        changes.push(Change::Text(line.clone()));
        if y == 0 {
            changes.push(AttributeChange::Intensity(termwiz::cell::Intensity::Normal).into());
        }
    }

    changes.push(Change::CursorPosition {
        x: Position::Absolute(x_pos),
        y: Position::Absolute(button_row),
    });

    if active == ActiveButton::Approve {
        changes.push(AttributeChange::Reverse(true).into());
    }
    changes.push(" [A]pprove ".into());
    if active == ActiveButton::Approve {
        changes.push(AttributeChange::Reverse(false).into());
    }

    changes.push("    ".into());

    if active == ActiveButton::Reject {
        changes.push(AttributeChange::Reverse(true).into());
    }
    changes.push(" [R]eject ".into());
    if active == ActiveButton::Reject {
        changes.push(AttributeChange::Reverse(false).into());
    }

    term.render(&changes)?;
    term.flush()?;

    // Make sure any mouse highlight state can be computed.
    // (We already computed button bounds; this function just draws.)
    let _ = (approve_x, approve_w, reject_x, reject_w);

    Ok(())
}

pub fn lucidity_pair_approve_overlay(
    mut term: TermWizTerminal,
    request: lucidity_pairing::PairingRequest,
) -> anyhow::Result<bool> {
    term.set_raw_mode()?;
    term.no_grab_mouse_in_raw_mode();

    let fingerprint = request.mobile_public_key.fingerprint_short();

    let lines = vec![
        "Lucidity pairing request".to_string(),
        "".to_string(),
        format!("Email:  {}", request.user_email),
        format!("Device: {}", request.device_name),
        format!("Key:    {}", fingerprint),
        "".to_string(),
        "Approve adds this device to your trust list.".to_string(),
    ];

    let size = term.get_screen_size()?;
    let x_pos = size.cols * 10 / 100;
    let content_rows = lines.len() + 1;
    let top_row = (size.rows.saturating_sub(content_rows)) / 2;
    let button_row = top_row + lines.len() + 1;

    let approve_x = x_pos;
    let approve_w = 11;
    let reject_x = approve_x + approve_w + 4;
    let reject_w = 10;

    let mut active = ActiveButton::None;

    render(
        &mut term,
        &lines,
        x_pos,
        top_row,
        button_row,
        active,
        approve_x,
        approve_w,
        reject_x,
        reject_w,
    )?;

    while let Ok(Some(event)) = term.poll_input(None) {
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('a' | 'A'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Enter,
                ..
            }) => {
                return Ok(true);
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('r' | 'R'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            }) => {
                return Ok(false);
            }
            InputEvent::Mouse(MouseEvent {
                x,
                y,
                mouse_buttons,
                ..
            }) => {
                let x = x as usize;
                let y = y as usize;

                if y == button_row && x >= approve_x && x < approve_x + approve_w {
                    active = ActiveButton::Approve;
                    if mouse_buttons == MouseButtons::LEFT {
                        return Ok(true);
                    }
                } else if y == button_row && x >= reject_x && x < reject_x + reject_w {
                    active = ActiveButton::Reject;
                    if mouse_buttons == MouseButtons::LEFT {
                        return Ok(false);
                    }
                } else {
                    active = ActiveButton::None;
                }

                if mouse_buttons != MouseButtons::NONE {
                    return Ok(false);
                }
            }
            _ => {}
        }

        render(
            &mut term,
            &lines,
            x_pos,
            top_row,
            button_row,
            active,
            approve_x,
            approve_w,
            reject_x,
            reject_w,
        )?;
    }

    Ok(false)
}
