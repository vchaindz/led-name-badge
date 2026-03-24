/// Text to bitmap renderer

use crate::font::{get_char_bitmap, get_icon};

/// Rendered bitmap data
pub struct Bitmap {
    pub data: Vec<u8>,
    pub width_columns: usize,  // Number of byte-columns (each 8 pixels wide)
}

impl Bitmap {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            width_columns: 0,
        }
    }

    /// Append character bitmap
    pub fn append_char(&mut self, ch: char) {
        if let Some(bitmap) = get_char_bitmap(ch) {
            self.data.extend_from_slice(bitmap);
            self.width_columns += 1;
        } else {
            // Unknown character - use space
            self.data.extend_from_slice(&[0u8; 11]);
            self.width_columns += 1;
        }
    }

    /// Append icon by name
    pub fn append_icon(&mut self, name: &str) {
        if let Some(icon) = get_icon(name) {
            self.data.extend_from_slice(icon.data);
            self.width_columns += icon.width_columns;
        }
    }

    /// Append raw bitmap data
    pub fn append_raw(&mut self, data: &[u8], columns: usize) {
        self.data.extend_from_slice(data);
        self.width_columns += columns;
    }
}

impl Default for Bitmap {
    fn default() -> Self {
        Self::new()
    }
}

/// Render text to bitmap, supporting :icon: notation
pub fn render_text(text: &str) -> Bitmap {
    let mut bitmap = Bitmap::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == ':' {
            // Check for :name: pattern
            let mut name = String::new();
            let mut found_end = false;

            for c in chars.by_ref() {
                if c == ':' {
                    found_end = true;
                    break;
                }
                name.push(c);
            }

            if found_end {
                if name.is_empty() {
                    // :: means literal colon
                    bitmap.append_char(':');
                } else if get_icon(&name).is_some() {
                    bitmap.append_icon(&name);
                } else {
                    // Unknown icon, render as text
                    bitmap.append_char(':');
                    for c in name.chars() {
                        bitmap.append_char(c);
                    }
                    bitmap.append_char(':');
                }
            } else {
                // No closing colon, render literally
                bitmap.append_char(':');
                for c in name.chars() {
                    bitmap.append_char(c);
                }
            }
        } else {
            bitmap.append_char(ch);
        }
    }

    bitmap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_hello() {
        let bitmap = render_text("hello");
        assert_eq!(bitmap.width_columns, 5);
        assert_eq!(bitmap.data.len(), 5 * 11);
    }

    #[test]
    fn test_render_with_icon() {
        let bitmap = render_text("I :heart: you");
        // "I " (2) + heart (1) + " you" (4) = 7
        assert_eq!(bitmap.width_columns, 7);
    }

    #[test]
    fn test_escaped_colon() {
        let bitmap = render_text("a::b");
        // "a" (1) + ":" (1) + "b" (1) = 3
        assert_eq!(bitmap.width_columns, 3);
    }
}
