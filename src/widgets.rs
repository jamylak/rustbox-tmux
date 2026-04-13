const GIT_SECTION_STUB: &str = "#[fg=colour142]▒  main";
const FORGE_SECTION_STUB: &str = "#[fg=colour214]▒  --";
const METRICS_SECTION_STUB: &str = "#[fg=colour109]▒ 🧠 --% #[fg=colour108]💾 --%";
const SHOW_FORGE_SECTION: bool = false;

pub fn git_section() -> &'static str {
    GIT_SECTION_STUB
}

pub fn forge_section() -> &'static str {
    if SHOW_FORGE_SECTION {
        FORGE_SECTION_STUB
    } else {
        ""
    }
}

pub fn metrics_section() -> &'static str {
    METRICS_SECTION_STUB
}

#[cfg(test)]
mod tests {
    use super::{forge_section, git_section, metrics_section, FORGE_SECTION_STUB, GIT_SECTION_STUB, METRICS_SECTION_STUB, SHOW_FORGE_SECTION};

    #[test]
    fn builds_current_widget_sections() {
        assert_eq!(git_section(), GIT_SECTION_STUB);
        assert_eq!(metrics_section(), METRICS_SECTION_STUB);

        if SHOW_FORGE_SECTION {
            assert_eq!(forge_section(), FORGE_SECTION_STUB);
        } else {
            assert_eq!(forge_section(), "");
        }
    }
}
