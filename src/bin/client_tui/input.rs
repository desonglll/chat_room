//! UTF-8-safe single-line editing used by TUI forms and composers.

use crossterm::event::{KeyCode, KeyEvent};

#[derive(Clone, Debug, Default)]
pub struct TextField {
    value: String,
    cursor: usize,
    masked: bool,
}

impl TextField {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self {
            value,
            cursor,
            masked: false,
        }
    }

    pub fn password() -> Self {
        Self {
            masked: true,
            ..Self::default()
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn display_value(&self) -> String {
        if self.masked {
            "*".repeat(self.value.chars().count())
        } else {
            self.value.clone()
        }
    }

    pub fn displayed_prefix(&self) -> String {
        self.display_value().chars().take(self.cursor).collect()
    }

    #[cfg(test)]
    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.chars().count();
    }

    pub fn take(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.value)
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(character)
                if !key.modifiers.intersects(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::ALT,
                ) =>
            {
                let byte = self.byte_index(self.cursor);
                self.value.insert(byte, character);
                self.cursor += 1;
                true
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let start = self.byte_index(self.cursor - 1);
                let end = self.byte_index(self.cursor);
                self.value.replace_range(start..end, "");
                self.cursor -= 1;
                true
            }
            KeyCode::Delete if self.cursor < self.value.chars().count() => {
                let start = self.byte_index(self.cursor);
                let end = self.byte_index(self.cursor + 1);
                self.value.replace_range(start..end, "");
                true
            }
            KeyCode::Left if self.cursor > 0 => {
                self.cursor -= 1;
                true
            }
            KeyCode::Right if self.cursor < self.value.chars().count() => {
                self.cursor += 1;
                true
            }
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.value.chars().count();
                true
            }
            _ => false,
        }
    }

    fn byte_index(&self, character_index: usize) -> usize {
        self.value
            .char_indices()
            .nth(character_index)
            .map_or(self.value.len(), |(index, _)| index)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn edits_cjk_at_character_boundaries() {
        let mut field = TextField::new("你好");
        field.handle_key(key(KeyCode::Left));
        field.handle_key(key(KeyCode::Char('，')));
        field.handle_key(key(KeyCode::Backspace));
        assert_eq!(field.value(), "你好");
    }

    #[test]
    fn password_display_never_exposes_value() {
        let mut field = TextField::password();
        field.set("secret");
        assert_eq!(field.display_value(), "******");
        assert!(!field.display_value().contains("secret"));
    }
}
