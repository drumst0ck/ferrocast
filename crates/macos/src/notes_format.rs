//! Formatting actions of the Notes editor, shared by the key shortcuts, the `/` menu, the Format
//! Bar and the action panel.

/// One formatting action. Inline ones toggle on the selection; block ones change the paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bold,
    Italic,
    Strikethrough,
    Underline,
    Code,
    Link,
    Heading(u8),
    CodeBlock,
    Quote,
    BulletList,
    OrderedList,
    TaskList,
    Rule,
}

impl Format {
    /// Every action in Format Bar order.
    pub const ALL: [Format; 15] = [
        Format::Bold,
        Format::Italic,
        Format::Underline,
        Format::Strikethrough,
        Format::Code,
        Format::Link,
        Format::Heading(1),
        Format::Heading(2),
        Format::Heading(3),
        Format::BulletList,
        Format::OrderedList,
        Format::TaskList,
        Format::Quote,
        Format::CodeBlock,
        Format::Rule,
    ];

    /// Human name shown in menus and tooltips.
    pub fn name(self) -> &'static str {
        match self {
            Format::Bold => "Bold",
            Format::Italic => "Italic",
            Format::Strikethrough => "Strikethrough",
            Format::Underline => "Underline",
            Format::Code => "Code",
            Format::Link => "Link",
            Format::Heading(1) => "Heading 1",
            Format::Heading(2) => "Heading 2",
            Format::Heading(_) => "Heading 3",
            Format::CodeBlock => "Code Block",
            Format::Quote => "Blockquote",
            Format::BulletList => "Bullet List",
            Format::OrderedList => "Ordered List",
            Format::TaskList => "Task List",
            Format::Rule => "Horizontal Rule",
        }
    }

    /// Shortcut shown next to the name (macOS glyphs), `None` when there is none.
    pub fn shortcut(self) -> Option<&'static str> {
        Some(match self {
            Format::Bold => "\u{2318}B",
            Format::Italic => "\u{2318}I",
            Format::Strikethrough => "\u{21E7}\u{2318}S",
            Format::Underline => "\u{2318}U",
            Format::Code => "\u{2318}E",
            Format::Link => "\u{2318}L",
            Format::Heading(1) => "\u{2325}\u{2318}1",
            Format::Heading(2) => "\u{2325}\u{2318}2",
            Format::Heading(_) => "\u{2325}\u{2318}3",
            Format::CodeBlock => "\u{2325}\u{2318}C",
            Format::Quote => "\u{21E7}\u{2318}B",
            Format::OrderedList => "\u{21E7}\u{2318}7",
            Format::BulletList => "\u{21E7}\u{2318}8",
            Format::TaskList => "\u{21E7}\u{2318}9",
            Format::Rule => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_a_name_and_the_spec_shortcuts() {
        assert_eq!(Format::ALL.len(), 15);
        assert!(Format::ALL.iter().all(|f| !f.name().is_empty()));
        assert_eq!(Format::Bold.shortcut(), Some("\u{2318}B"));
        assert_eq!(Format::Heading(2).shortcut(), Some("\u{2325}\u{2318}2"));
        assert_eq!(Format::Rule.shortcut(), None);
    }
}
