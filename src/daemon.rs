use std::thread;
use std::time::Duration;

use crate::render::{
    RenderState, Renderer, DEFAULT_FORGE_SECTION, DEFAULT_GIT_SECTION, DEFAULT_METRICS_SECTION,
};
use crate::tmux::{publish_status, refresh_status_line, STATUS_OPTION};

const IDLE_LOOP_SLEEP_SECS: u64 = 60;

pub fn run_daemon() -> Result<(), String> {
    let state = build_render_state();
    let mut renderer = Renderer::new();
    publish_status(renderer.render(&state))?;

    // Force one initial status redraw so attached clients pick up the first
    // published value immediately. Steady-state updates should rely on tmux
    // redrawing `#{@rustbox_status_right}` on its normal cadence.
    refresh_status_line()?;

    log_startup();

    run_idle_loop();
}

fn build_render_state() -> RenderState {
    RenderState {
        git_section: build_git_section(),
        forge_section: build_forge_section(),
        metrics_section: build_metrics_section(),
    }
}

fn build_git_section() -> &'static str {
    DEFAULT_GIT_SECTION
}

fn build_forge_section() -> &'static str {
    DEFAULT_FORGE_SECTION
}

fn build_metrics_section() -> &'static str {
    DEFAULT_METRICS_SECTION
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
