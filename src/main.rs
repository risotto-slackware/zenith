use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stdout;
use std::sync::Arc;
use tokio::sync::watch;

mod system;
mod ui;

use system::SystemSnapshot;

#[tokio::main]
async fn main() -> Result<()> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal: Terminal<CrosstermBackend<Stdout>> = Terminal::new(backend)?;

    // Channel to send snapshots to UI
    let (tx, rx) = watch::channel(SystemSnapshot::default());
    let rx = Arc::new(tokio::sync::Mutex::new(rx));

    // Spawn updater task
    let updater = tokio::spawn(system::updater(tx));

    // Run UI loop
    let ui_res = ui::run_ui(&mut terminal, rx).await;

    // Cleanup
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), DisableMouseCapture)?;
    terminal.show_cursor()?;

    // Ensure updater finishes
    let _ = updater.abort();

    ui_res
}
