//! Clipboard memory system (Phase 2).
//!
//! Pattern from pluggedin-mcp-proxy: dual stack (push/pop with auto-index)
//! and key-value (set/get/delete by name) memory for agent persistence
//! across tool calls.

use std::collections::HashMap;

/// In-memory clipboard store for agent persistence.
#[derive(Debug, Default)]
pub struct Clipboard {
    /// Named key-value entries.
    entries: HashMap<String, String>,
    /// Stack entries (auto-indexed).
    stack: Vec<String>,
    next_index: usize,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            stack: Vec::new(),
            next_index: 0,
        }
    }

    /// Set a named clipboard entry.
    pub fn set(&mut self, name: &str, value: String) {
        self.entries.insert(name.to_string(), value);
    }

    /// Get a named clipboard entry.
    pub fn get(&self, name: &str) -> Option<&String> {
        self.entries.get(name)
    }

    /// Delete a named clipboard entry.
    pub fn delete(&mut self, name: &str) -> bool {
        self.entries.remove(name).is_some()
    }

    /// Push a value onto the stack (auto-indexed).
    pub fn push(&mut self, value: String) -> usize {
        let idx = self.next_index;
        self.stack.push(value);
        self.next_index += 1;
        idx
    }

    /// Pop the most recently pushed value.
    pub fn pop(&mut self) -> Option<String> {
        self.stack.pop()
    }

    /// List all named entries.
    pub fn list_entries(&self) -> Vec<(&String, &String)> {
        self.entries.iter().collect()
    }

    /// Get stack size.
    pub fn stack_size(&self) -> usize {
        self.stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_set_get() {
        let mut cb = Clipboard::new();
        cb.set("key1", "value1".to_string());
        assert_eq!(cb.get("key1"), Some(&"value1".to_string()));
        assert_eq!(cb.get("nonexistent"), None);
    }

    #[test]
    fn test_clipboard_delete() {
        let mut cb = Clipboard::new();
        cb.set("key1", "value1".to_string());
        assert!(cb.delete("key1"));
        assert!(!cb.delete("key1"));
    }

    #[test]
    fn test_clipboard_push_pop() {
        let mut cb = Clipboard::new();
        cb.push("first".to_string());
        cb.push("second".to_string());
        assert_eq!(cb.pop(), Some("second".to_string()));
        assert_eq!(cb.pop(), Some("first".to_string()));
        assert_eq!(cb.pop(), None);
    }

    #[test]
    fn test_clipboard_list() {
        let mut cb = Clipboard::new();
        cb.set("a", "1".to_string());
        cb.set("b", "2".to_string());
        let mut entries = cb.list_entries();
        entries.sort_by_key(|(k, _)| *k);
        assert_eq!(entries.len(), 2);
    }
}
