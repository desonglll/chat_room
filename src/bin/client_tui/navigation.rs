//! Consistent list navigation shared by TUI views and dialogs.

use crossterm::event::KeyCode;

const PAGE_STEP: usize = 5;

pub(super) fn move_selection(index: &mut usize, len: usize, key: KeyCode) -> bool {
    let next = match key {
        KeyCode::Up | KeyCode::Char('k') => index.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => index.saturating_add(1),
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

    #[test]
    fn supports_vim_arrows_boundaries_and_pages() {
        let mut index = 5;
        assert!(move_selection(&mut index, 12, KeyCode::Char('k')));
        assert_eq!(index, 4);
        assert!(move_selection(&mut index, 12, KeyCode::PageDown));
        assert_eq!(index, 9);
        assert!(move_selection(&mut index, 12, KeyCode::End));
        assert_eq!(index, 11);
        assert!(move_selection(&mut index, 12, KeyCode::Down));
        assert_eq!(index, 11);
        assert!(move_selection(&mut index, 12, KeyCode::Home));
        assert_eq!(index, 0);
    }

    #[test]
    fn ignores_non_navigation_keys() {
        let mut index = 2;
        assert!(!move_selection(&mut index, 4, KeyCode::Enter));
        assert_eq!(index, 2);
    }
}
