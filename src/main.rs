mod app;
mod db;
mod domain;
mod event;
mod filter;
mod forms;
mod input;
mod seed;
mod state;
mod ui;

use std::io::{Stdout, stdout};

use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

fn setup_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout()))?)
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
}

/// Restore the terminal before printing panic info so the shell is never left broken.
fn install_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        hook(info);
    }));
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_path = db::default_db_path()?;
    let database = db::Db::connect(&db_path).await?;

    let args: Vec<String> = std::env::args().collect();
    if let Some(idx) = args.iter().position(|a| a == "--seed") {
        let count: usize = args.get(idx + 1).and_then(|v| v.parse().ok()).unwrap_or(50);
        seed::seed(&database, count).await?;
        println!("Seeded {count} tasks into {}", db_path.display());
        return Ok(());
    }

    let (tx, rx) = app::channel();
    let app = app::App::new(database, tx);
    app.start();

    install_panic_hook();
    let mut terminal = setup_terminal()?;
    let result = event::run(&mut terminal, app, rx).await;
    restore_terminal();
    result
}
