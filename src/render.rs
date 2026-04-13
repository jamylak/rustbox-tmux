const STATUS_SEPARATOR: &str = "#[fg=colour244] | ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderState {
    pub git_section: &'static str,
    pub forge_section: &'static str,
    pub metrics_section: &'static str,
}

#[derive(Debug, Default)]
pub struct Renderer {
    output: String,
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render<'a>(&'a mut self, state: &RenderState) -> &'a str {
        render_status(state, &mut self.output);
        self.output.as_str()
    }
}

pub fn render_to_stdout(state: &RenderState) {
    let mut renderer = Renderer::new();
    println!("{}", renderer.render(state));
}

pub fn render_status(state: &RenderState, output: &mut String) {
    output.clear();
    append_status(state, output);
}

fn append_status(state: &RenderState, output: &mut String) {
    append_section(output, state.git_section);
    append_section(output, state.forge_section);
    append_section(output, state.metrics_section);
}

fn append_section(output: &mut String, section: &str) {
    if section.is_empty() {
        return;
    }

    if !output.is_empty() {
        output.push_str(STATUS_SEPARATOR);
    }

    output.push_str(section);
}

#[cfg(test)]
mod tests {
    use super::{render_status, RenderState, Renderer};

    #[test]
    fn renders_static_status() {
        let mut output = String::from("stale");
        render_status(
            &RenderState {
                git_section: "#[fg=colour142]▒  main",
                forge_section: "#[fg=colour214]▒  --",
                metrics_section: "#[fg=colour109]▒ 🧠 --% #[fg=colour108]💾 --%",
            },
            &mut output,
        );

        assert_eq!(
            output,
            "#[fg=colour142]▒  main#[fg=colour244] | #[fg=colour214]▒  --#[fg=colour244] | #[fg=colour109]▒ 🧠 --% #[fg=colour108]💾 --%"
        );
    }

    #[test]
    fn renderer_reuses_its_buffer() {
        let state = RenderState {
            git_section: "#[fg=colour142]▒  main",
            forge_section: "#[fg=colour214]▒  --",
            metrics_section: "#[fg=colour109]▒ 🧠 --% #[fg=colour108]💾 --%",
        };
        let mut renderer = Renderer::new();
        let first_ptr = renderer.render(&state).as_ptr();
        let second_ptr = renderer.render(&state).as_ptr();

        assert_eq!(
            renderer.render(&state),
            "#[fg=colour142]▒  main#[fg=colour244] | #[fg=colour214]▒  --#[fg=colour244] | #[fg=colour109]▒ 🧠 --% #[fg=colour108]💾 --%"
        );
        assert_eq!(first_ptr, second_ptr);
    }

    #[test]
    fn renders_sections_from_state() {
        let state = RenderState {
            git_section: "git",
            forge_section: "forge",
            metrics_section: "metrics",
        };
        let mut output = String::new();

        render_status(&state, &mut output);

        assert_eq!(output, "git#[fg=colour244] | forge#[fg=colour244] | metrics");
    }

    #[test]
    fn skips_empty_sections_without_extra_separators() {
        let state = RenderState {
            git_section: "git",
            forge_section: "",
            metrics_section: "metrics",
        };
        let mut output = String::new();

        render_status(&state, &mut output);

        assert_eq!(output, "git#[fg=colour244] | metrics");
    }
}
