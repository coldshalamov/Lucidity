use clipboard_win::{get_clipboard_string, formats};
use std::thread;
use std::time::Duration;
use log::debug;

pub fn start_clipboard_monitor<F>(callback: F) 
where F: Fn(String) + Send + 'static 
{
    thread::spawn(move || {
        let mut last_clipboard = String::new();
        
        loop {
            thread::sleep(Duration::from_millis(1000));
            
            match get_clipboard_string() {
                Ok(text) => {
                    if !text.is_empty() && text != last_clipboard {
                        debug!("Clipboard changed on host");
                        last_clipboard = text.clone();
                        callback(text);
                    }
                }
                Err(_) => {
                    // This often happens if the clipboard is empty or non-text
                    // We just ignore it
                }
            }
        }
    });
}
