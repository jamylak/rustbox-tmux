const STATIC_STATUS: &str = "#[fg=green]rustbox-tmux bootstrap";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderState;

pub fn render_current_status(output: &mut String) {
    render_status(&RenderState, output);
}

pub fn render_status(state: &RenderState, output: &mut String) {
    output.clear();
    append_status(state, output);
}

fn append_status(_state: &RenderState, output: &mut String) {
    output.push_str(STATIC_STATUS);
}

#[cfg(test)]
mod tests {
    use super::{render_current_status, render_status, RenderState};

    #[test]
    fn renders_static_status() {
        let mut output = String::from("stale");
        render_status(&RenderState, &mut output);

        assert_eq!(output, "#[fg=green]rustbox-tmux bootstrap");
    }

    #[test]
    fn renders_current_status() {
        let mut output = String::new();
        render_current_status(&mut output);

        assert_eq!(output, "#[fg=green]rustbox-tmux bootstrap");
    }
}
