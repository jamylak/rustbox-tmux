const STATIC_STATUS: &str = "#[fg=green]rustbox-tmux bootstrap";

pub fn render_status() -> &'static str {
    STATIC_STATUS
}

#[cfg(test)]
mod tests {
    use super::render_status;

    #[test]
    fn renders_static_status() {
        assert_eq!(render_status(), "#[fg=green]rustbox-tmux bootstrap");
    }
}
