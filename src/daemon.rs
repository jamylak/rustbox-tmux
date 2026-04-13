use std::thread;
use std::time::Duration;

use crate::render::{RenderState, Renderer};
use crate::tmux::{publish_status, refresh_status_line, STATUS_OPTION};

const IDLE_LOOP_SLEEP_SECS: u64 = 60;
const GIT_SECTION_STUB: &str = "#[fg=colour142]▒  main";
const FORGE_SECTION_STUB: &str = "#[fg=colour214]▒  --";
const METRICS_SECTION_STUB: &str = "#[fg=colour109]▒ 🧠 --% #[fg=colour108]💾 --%";
const SHOW_FORGE_SECTION: bool = false;

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
        git_section: build_git_section(),
        forge_section: build_forge_section(),
        metrics_section: build_metrics_section(),
    }
}

fn build_git_section() -> &'static str {
    GIT_SECTION_STUB
}

fn build_forge_section() -> &'static str {
    if SHOW_FORGE_SECTION {
        FORGE_SECTION_STUB
    } else {
        ""
    }
}

fn build_metrics_section() -> &'static str {
    METRICS_SECTION_STUB
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
    use super::{current_render_state, GIT_SECTION_STUB, METRICS_SECTION_STUB, SHOW_FORGE_SECTION, FORGE_SECTION_STUB};

    #[test]
    fn builds_render_state_from_current_sections() {
        let state = current_render_state();

        assert_eq!(state.git_section, GIT_SECTION_STUB);
        assert_eq!(state.metrics_section, METRICS_SECTION_STUB);

        if SHOW_FORGE_SECTION {
            assert_eq!(state.forge_section, FORGE_SECTION_STUB);
        } else {
            assert_eq!(state.forge_section, "");
        }
    }
}
