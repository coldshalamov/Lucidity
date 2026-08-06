#![cfg_attr(not(test), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    agent_terminal::chrome::run()
}
