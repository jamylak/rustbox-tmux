use std::thread;
use std::time::Duration;

use crate::render::{RenderState, Renderer};
use crate::tmux::{publish_status, refresh_status_line, STATUS_OPTION};

const IDLE_LOOP_SLEEP_SECS: u64 = 60;

pub fn run_daemon() -> Result<(), String> {
    let state = RenderState::right_bar_stub();
    let mut renderer = Renderer::new();
    publish_status(renderer.render(&state))?;

    // Force one initial status redraw so attached clients pick up the first
    // published value immediately. Steady-state updates should rely on tmux
    // redrawing `#{@rustbox_status_right}` on its normal cadence.
    refresh_status_line()?;

    log_startup();

    run_idle_loop();
}

fn log_startup() {
    eprintln!("rustbox-tmuxd started");
    eprintln!("published initial status to {STATUS_OPTION}");
}

fn run_idle_loop() -> ! {
    loop {
        thread::sleep(Duration::from_secs(IDLE_LOOP_SLEEP_SECS));
    }
}
