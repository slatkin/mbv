use super::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(super) fn handle_key_home_search(&mut self, key: KeyEvent) -> Option<bool> {
        if self.library_tab != 0 || !self.search.is_open() || self.context_menu_open() {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            match key.code {
                KeyCode::Left | KeyCode::Right => {
                    if let Some(hs) = self.search.state_mut() {
                        let n = hs.available_types().len() + 1;
                        if n > 1 {
                            hs.type_filter = if key.code == KeyCode::Right {
                                (hs.type_filter + 1) % n
                            } else {
                                (hs.type_filter + n - 1) % n
                            };
                            hs.cursor = 0;
                            hs.scroll = 0;
                        }
                    }
                    return Some(false);
                }
                _ => return None,
            }
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return None;
        }
        let input_focused = self.search.state().is_none_or(|s| s.input_focused);
        match key.code {
            KeyCode::Esc => {
                self.search.close();
            }
            KeyCode::Tab => {
                if let Some(hs) = self.search.state_mut() {
                    hs.input_focused = !hs.input_focused;
                }
            }
            KeyCode::Backspace if input_focused => {
                let empty = self.search.state().is_none_or(|s| s.query.is_empty());
                if empty {
                    self.search.close();
                } else {
                    self.search.state_mut().unwrap().query.pop();
                }
            }
            KeyCode::Up => {
                if let Some(hs) = self.search.state_mut() {
                    hs.cursor = hs.cursor.saturating_sub(1);
                    if hs.cursor < hs.scroll {
                        hs.scroll = hs.cursor;
                    }
                }
            }
            KeyCode::Down => {
                if let Some(hs) = self.search.state_mut() {
                    let max = hs.filtered_count().saturating_sub(1);
                    hs.cursor = (hs.cursor + 1).min(max);
                }
            }
            KeyCode::Enter => {
                let (query, last_query, loading, has_results) = self
                    .search
                    .state()
                    .as_ref()
                    .map(|hs| {
                        (
                            hs.query.clone(),
                            hs.last_query.clone(),
                            hs.loading,
                            !hs.results.is_empty(),
                        )
                    })
                    .unwrap_or_default();
                if loading {
                    return Some(false);
                }
                if !input_focused {
                    if has_results {
                        self.select_home();
                    }
                    return Some(false);
                }
                if query.is_empty() {
                    return Some(false);
                }
                if query != last_query {
                    self.search.prepare_query(&query);
                    self.spawn_global_search(query);
                } else if has_results {
                    self.select_home();
                }
            }
            KeyCode::Char('q') if !input_focused && key.modifiers.is_empty() => {
                return Some(self.try_quit());
            }
            KeyCode::Char(c) => {
                if let Some(hs) = self.search.state_mut() {
                    hs.input_focused = true;
                    hs.query.push(c);
                }
            }
            _ => {}
        }
        Some(false)
    }

    pub(super) fn handle_key_context_menu(&mut self, key: KeyEvent) -> Option<bool> {
        self.context_menu.as_ref()?;
        match key.code {
            KeyCode::Esc => {
                self.context_menu = None;
                self.force_clear = true;
            }
            KeyCode::Up => {
                if let Some(m) = &mut self.context_menu {
                    m.move_cursor(-1);
                }
            }
            KeyCode::Down => {
                if let Some(m) = &mut self.context_menu {
                    m.move_cursor(1);
                }
            }
            KeyCode::Enter => {
                if let Some(m) = self.context_menu.take() {
                    self.force_clear = true;
                    let action = m
                        .entries
                        .get(m.cursor)
                        .and_then(|entry| entry.action.clone());
                    self.execute_context_action(action);
                }
            }
            _ => {}
        }
        Some(false)
    }
}
