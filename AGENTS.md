# rustbox-tmux Agent Notes

## Commit Style

- Group commits by logical groups of changes
- Prefer colourful, high-signal commit subjects when they fit the change.
- Emojis are encouraged when they add meaning instead of noise.
- Commit bodies should read well in Markdown:
  - short paragraphs or flat bullets are preferred
  - inline code should be used for important symbols such as `run_daemon()` or `publish_to_tmux()`
  - mention the main user-visible or architecture-visible effect, not a file-by-file changelog
- Keep commit messages intentional and specific even when they are playful.
