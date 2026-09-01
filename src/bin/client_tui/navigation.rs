//! Consistent list navigation shared by TUI views and dialogs.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const PAGE_STEP: usize = 5;

pub(super) fn is_plain(key: KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

pub(super) fn move_selection(index: &mut usize, len: usize, key: KeyEvent) -> bool {
    let next = match key.code {
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            index.saturating_sub(1)
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            index.saturating_add(1)
        }
        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::ALT) => {
            index.saturating_sub(PAGE_STEP)
        }
        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            index.saturating_add(PAGE_STEP)
        }
        KeyCode::Char('<') if key.modifiers.contains(KeyModifiers::ALT) => 0,
        KeyCode::Char('>') if key.modifiers.contains(KeyModifiers::ALT) => len.saturating_sub(1),
        KeyCode::Up => index.saturating_sub(1),
        KeyCode::Down => index.saturating_add(1),
        KeyCode::Char('k') if is_plain(key) => index.saturating_sub(1),
        KeyCode::Char('j') if is_plain(key) => index.saturating_add(1),
        KeyCode::Home => 0,
        KeyCode::End => len.saturating_sub(1),
        KeyCode::PageUp => index.saturating_sub(PAGE_STEP),
        KeyCode::PageDown => index.saturating_add(PAGE_STEP),
        _ => return false,
    };
    *index = next.min(len.saturating_sub(1));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn supports_vim_arrows_boundaries_and_pages() {
        let mut index = 5;
        assert!(move_selection(&mut index, 12, key(KeyCode::Char('k'))));
        assert_eq!(index, 4);
        assert!(move_selection(&mut index, 12, key(KeyCode::PageDown)));
        assert_eq!(index, 9);
        assert!(move_selection(&mut index, 12, key(KeyCode::End)));
        assert_eq!(index, 11);
        assert!(move_selection(&mut index, 12, key(KeyCode::Down)));
        assert_eq!(index, 11);
        assert!(move_selection(&mut index, 12, key(KeyCode::Home)));
        assert_eq!(index, 0);
    }

    #[test]
    fn ignores_non_navigation_keys() {
        let mut index = 2;
        assert!(!move_selection(&mut index, 4, key(KeyCode::Enter)));
        assert_eq!(index, 2);
    }

    #[test]
    fn supports_emacs_navigation_keys() {
        let mut index = 5;
        assert!(move_selection(
            &mut index,
            12,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        ));
        assert_eq!(index, 4);
        assert!(move_selection(
            &mut index,
            12,
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
        ));
        assert_eq!(index, 9);
        assert!(move_selection(
            &mut index,
            12,
            KeyEvent::new(KeyCode::Char('<'), KeyModifiers::ALT),
        ));
        assert_eq!(index, 0);
    }
}
