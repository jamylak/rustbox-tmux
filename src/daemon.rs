use std::thread;
use std::time::Duration;

use crate::render::{RenderState, Renderer};
use crate::tmux::{publish_status, refresh_status_line, STATUS_OPTION};
use crate::widgets::{forge_section, git_section_string, metrics_section_string};

const IDLE_LOOP_SLEEP_SECS: u64 = 60;

pub fn run_daemon() -> Result<(), String> {
    let state = current_render_state();
    let mut renderer = Renderer::new();
    publish_status(renderer.render(&state))?;

    // Force one initial status redraw so attached clients pick up the first
    // published value immediately. Steady-state updates should rely on tmux
    // redrawing `#{@rustbox_status_right}` on its normal cadence.
    refresh_status_line()?;

    log_startup();

    run_idle_loop();
}

pub fn current_render_state() -> RenderState {
    RenderState {
        git_section: git_section_string(),
        forge_section: forge_section().to_string(),
        metrics_section: metrics_section_string(),
    }
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

#[cfg(test)]
mod tests {
    use super::current_render_state;
    use crate::widgets::{forge_section, git_section_string, metrics_section_string};

    #[test]
    fn builds_render_state_from_current_sections() {
        let state = current_render_state();

        assert_eq!(state.git_section, git_section_string());
        assert_eq!(state.forge_section, forge_section());
        assert_eq!(state.metrics_section, metrics_section_string());
    }
}
