use crossterm::event::{Event, KeyEvent};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::MissedTickBehavior;

/// Events consumed by the application main loop.
pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    Resize(u16, u16),
}

/// Receives application events produced by a background task.
pub struct EventHandler {
    rx: mpsc::Receiver<AppEvent>,
    tick_rate_tx: watch::Sender<u64>,
}

impl EventHandler {
    /// Spawn the background event task.
    ///
    /// The task emits `AppEvent::Tick` on the configured interval and polls
    /// crossterm for keyboard and resize events without blocking.
    pub fn new(tick_rate_ms: u64) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let (tick_rate_tx, mut tick_rate_rx) = watch::channel(tick_rate_ms.max(1));

        tokio::spawn(async move {
            let mut interval = new_interval(*tick_rate_rx.borrow());

            loop {
                // Drain any pending terminal events without blocking.
                loop {
                    match crossterm::event::poll(Duration::ZERO) {
                        Ok(true) => match crossterm::event::read() {
                            Ok(Event::Key(key)) => {
                                if tx.send(AppEvent::Key(key)).await.is_err() {
                                    return;
                                }
                            }
                            Ok(Event::Resize(width, height)) => {
                                if tx.send(AppEvent::Resize(width, height)).await.is_err() {
                                    return;
                                }
                            }
                            Ok(_) => {}
                            Err(_) => return,
                        },
                        Ok(false) => break,
                        Err(_) => return,
                    }
                }

                tokio::select! {
                    _ = interval.tick() => {
                        if tx.send(AppEvent::Tick).await.is_err() {
                            return;
                        }
                    }
                    result = tick_rate_rx.changed() => {
                        if result.is_err() {
                            return;
                        }
                        interval = new_interval(*tick_rate_rx.borrow());
                    }
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {}
                }
            }
        });

        Self { rx, tick_rate_tx }
    }

    /// Change the tick interval of the running event task.
    pub fn set_tick_rate(&self, tick_rate_ms: u64) {
        let _ = self.tick_rate_tx.send(tick_rate_ms.max(1));
    }

    /// Wait for the next application event.
    pub async fn next(&mut self) -> anyhow::Result<AppEvent> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("event channel closed"))
    }
}

fn new_interval(tick_rate_ms: u64) -> tokio::time::Interval {
    let mut interval = tokio::time::interval(Duration::from_millis(tick_rate_ms.max(1)));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    interval
}
