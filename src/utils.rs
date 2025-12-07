//! Utility helpers used across the application (formatting, loading data, images).
//! Keep helpers small and well-documented for readability.

use crate::models::Pokemon;
use std::error::Error;
use std::fs;

/// Format a Pokémon `name` into a human-friendly form.
///
/// Examples: `mr-mime` -> `Mr Mime`, `ho_oh` -> `Ho Oh`.
pub fn format_name(name: &str) -> String {
    let replaced = name.replace('-', " ").replace('_', " ");
    let parts: Vec<String> = replaced
        .split_whitespace()
        .map(|w| {
            let mut chs = w.chars();
            match chs.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chs.as_str().to_lowercase()
                }
            }
        })
        .collect();
    parts.join(" ")
}

pub fn text_to_lines(s: &str, width: usize) -> Vec<String> {
    // Wrap text into lines no longer than `width` (simple greedy algorithm).
    let mut lines = vec![];
    let mut current = String::new();
    for word in s.split_whitespace() {
        if current.len() + word.len() + 1 > width && !current.is_empty() {
            lines.push(current.clone());
            current.clear();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub fn load_data(path: &str) -> Result<Vec<Pokemon>, Box<dyn Error>> {
    let data = fs::read_to_string(path)?;
    let v: Vec<Pokemon> = serde_json::from_str(&data)?;
    Ok(v)
}

// `mock_ai_summary` removed — AI-generated summary was optional and is not used
// in the current UI. Keep other helpers small and focused.
