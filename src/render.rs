const STATIC_STATUS: &str = "#[fg=green]rustbox-tmux bootstrap";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderState;

#[derive(Debug, Default)]
pub struct Renderer {
    state: RenderState,
    output: String,
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&mut self) -> &str {
        render_status(&self.state, &mut self.output);
        self.output.as_str()
    }
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
    use super::{render_status, RenderState, Renderer};

    #[test]
    fn renders_static_status() {
        let mut output = String::from("stale");
        render_status(&RenderState, &mut output);

        assert_eq!(output, "#[fg=green]rustbox-tmux bootstrap");
    }

    #[test]
    fn renderer_reuses_its_buffer() {
        let mut renderer = Renderer::new();
        let first_ptr = renderer.render().as_ptr();
        let second_ptr = renderer.render().as_ptr();

        assert_eq!(renderer.render(), "#[fg=green]rustbox-tmux bootstrap");
        assert_eq!(first_ptr, second_ptr);
    }
}
