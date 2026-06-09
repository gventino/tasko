use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::app::{App, Message};
use crate::{Tui, input, ui};

/// Fully event-driven loop: no tick. The terminal is redrawn at most once per
/// wake-up, after draining every queued message (batched updates → one render).
pub async fn run(
    terminal: &mut Tui,
    mut app: App,
    mut rx: UnboundedReceiver<Message>,
) -> Result<()> {
    let mut events = EventStream::new();
    terminal.draw(|frame| ui::draw(&app, frame))?;

    while !app.should_quit {
        tokio::select! {
            ev = events.next() => match ev {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    if let Some(msg) = input::map_key(&app, key) {
                        app.update(msg);
                    }
                }
                Some(Ok(Event::Resize(..))) => app.mark_dirty(),
                Some(Ok(_)) => {}
                Some(Err(err)) => return Err(err.into()),
                None => break,
            },
            msg = rx.recv() => {
                if let Some(msg) = msg {
                    app.update(msg);
                }
            }
        }

        // Drain anything else already queued before paying for a render.
        while let Ok(msg) = rx.try_recv() {
            app.update(msg);
        }

        if app.take_dirty() && !app.should_quit {
            terminal.draw(|frame| ui::draw(&app, frame))?;
        }
    }
    Ok(())
}
