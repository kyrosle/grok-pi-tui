//! Exercise the production input path without compiling unrelated legacy unit fixtures.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use xai_grok_pager::views::prompt_widget::{PromptEvent, PromptWidget};

#[test]
fn delivered_modified_enter_replaces_selection() {
    for modifier in [KeyModifiers::SHIFT, KeyModifiers::ALT, KeyModifiers::SUPER] {
        let mut prompt = PromptWidget::new();
        prompt.set_text("alpha beta");
        prompt.set_cursor(5);
        prompt.handle_key(&KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT));
        assert!(prompt.textarea().selection_range().is_some());
        assert_eq!(
            prompt.handle_key(&KeyEvent::new(KeyCode::Enter, modifier)),
            PromptEvent::Edited
        );
        assert_eq!(prompt.text(), "\n beta");
        assert!(prompt.textarea().selection_range().is_none());
    }
}
