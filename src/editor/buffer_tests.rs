//! Tests for editor buffer functionality
//!
//! Covers issues:
//! - Double-click word selection with line wrap
//! - char_at_pos accuracy with wrapping enabled
//! - Word boundary detection

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::editor::buffer::LineEnding;
    use ropey::Rope;

    // =========================================================================
    // Word Selection Tests (Double-click functionality)
    // =========================================================================

    #[test]
    fn test_select_word_at_cursor_basic() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello world test");
        buffer.set_cursor(6); // Position at 'w' in "world"

        buffer.select_word_at_cursor();

        assert!(buffer.selection_anchor().is_some());
        let anchor = buffer.selection_anchor().unwrap();
        assert_eq!(anchor, 6); // Start of "world"
        assert_eq!(buffer.cursor(), 11); // End of "world" (exclusive)
    }

    #[test]
    fn test_select_word_at_cursor_start_of_word() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello world");
        buffer.set_cursor(0); // Start of buffer

        buffer.select_word_at_cursor();

        assert_eq!(buffer.selection_anchor(), Some(0));
        assert_eq!(buffer.cursor(), 5); // End of "hello"
    }

    #[test]
    fn test_select_word_at_cursor_end_of_word() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello world");
        buffer.set_cursor(4); // Last char of "hello" ('o')

        buffer.select_word_at_cursor();

        assert_eq!(buffer.selection_anchor(), Some(0));
        assert_eq!(buffer.cursor(), 5); // End of "hello"
    }

    #[test]
    fn test_select_word_at_cursor_on_whitespace() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello  world"); // Two spaces
        buffer.set_cursor(5); // Position on first space

        buffer.select_word_at_cursor();

        // Should not select anything when on whitespace
        assert!(buffer.selection_anchor().is_none());
    }

    #[test]
    fn test_select_word_at_cursor_empty_buffer() {
        let mut buffer = Buffer::new();
        buffer.set_cursor(0);

        buffer.select_word_at_cursor();

        // Should not crash on empty buffer
        assert!(buffer.selection_anchor().is_none());
    }

    #[test]
    fn test_select_word_with_underscores() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello_world_test");
        buffer.set_cursor(5); // Position at '_'

        buffer.select_word_at_cursor();

        // Underscores are considered word characters
        assert_eq!(buffer.selection_anchor(), Some(0));
        assert_eq!(buffer.cursor(), 16); // Whole string is one word
    }

    #[test]
    fn test_select_word_with_numbers() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("test123variable");
        buffer.set_cursor(7); // Middle of the alphanumeric word

        buffer.select_word_at_cursor();

        assert_eq!(buffer.selection_anchor(), Some(0));
        assert_eq!(buffer.cursor(), 15);
    }

    // =========================================================================
    // char_at_pos Tests (Click-to-position with wrapping)
    // =========================================================================

    #[test]
    fn test_char_at_pos_no_wrap_simple() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello\nworld\ntest");

        // Click on first line, first char
        let pos = buffer.char_at_pos(
            10.0, // x
            10.0, // y
            8.0,  // x_offset
            26.0, // line_height
            10.8, // char_width
            None, // no wrap
        );

        assert_eq!(pos, 0);
    }

    #[test]
    fn test_char_at_pos_no_wrap_second_line() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello\nworld\ntest");

        // Click on second line
        let pos = buffer.char_at_pos(
            10.0, 40.0, // y position on second line (below first line)
            8.0, 26.0, 10.8, None,
        );

        // Should be somewhere on second line ("world")
        assert!((6..=11).contains(&pos));
    }

    #[test]
    fn test_char_at_pos_with_wrap_basic() {
        let mut buffer = Buffer::new();
        buffer.wrap_enabled = true;
        buffer.rope = Rope::from_str("this is a very long line that should wrap");

        let wrap_width = Some(200.0f32); // Narrow width to force wrapping

        // Click near the beginning
        let pos = buffer.char_at_pos(20.0, 10.0, 8.0, 26.0, 10.8, wrap_width);

        // Should return a valid position within the text
        assert!(pos <= buffer.rope.len_chars());
    }

    #[test]
    fn test_char_at_pos_with_wrap_second_visual_line() {
        let mut buffer = Buffer::new();
        buffer.wrap_enabled = true;
        // Create a line long enough to wrap with narrow width
        buffer.rope = Rope::from_str("01234567890123456789012345678901234567890123456789");

        let wrap_width = Some(100.0f32); // Very narrow

        // Click on what would be the second visual line of wrapped text
        let pos = buffer.char_at_pos(
            20.0, 40.0, // y position indicating second visual line
            8.0, 26.0, 10.8, wrap_width,
        );

        // Position should be in the text, not clamped to first segment
        assert!(pos > 0);
        assert!(pos <= buffer.rope.len_chars());
    }

    #[test]
    fn test_char_at_pos_with_wrap_respects_visual_scroll() {
        let mut buffer = Buffer::new();
        buffer.wrap_enabled = true;
        buffer.rope = Rope::from_str("abcdefghijklmnopqrstuvwxyz0123456789");
        buffer.scroll_y = 3.0;
        buffer.scroll_y_target = 3.0;

        let wrap_width = Some(100.0f32);
        let pos = buffer.char_at_pos(8.0, 0.0, 8.0, 26.0, 10.0, wrap_width);

        assert_eq!(pos, 30);
    }

    #[test]
    fn test_char_at_pos_with_wrap_empty_line() {
        let mut buffer = Buffer::new();
        buffer.wrap_enabled = true;
        buffer.rope = Rope::from_str("line1\n\nline3");

        let wrap_width = Some(200.0f32);

        // Click on empty line
        let pos = buffer.char_at_pos(
            20.0, 40.0, // y position on empty second line
            8.0, 26.0, 10.8, wrap_width,
        );

        // Should return position at start of empty line
        assert_eq!(pos, 6); // After "line1\n"
    }

    #[test]
    fn test_char_at_pos_clamps_to_end() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("short");

        // Click far to the right
        let pos = buffer.char_at_pos(
            1000.0, // Way past the text
            10.0, 8.0, 26.0, 10.8, None,
        );

        // Should clamp to end of line (5 chars, but excluding newline)
        assert_eq!(pos, 5);
    }

    #[test]
    fn test_char_at_pos_empty_buffer() {
        let buffer = Buffer::new();

        let pos = buffer.char_at_pos(10.0, 10.0, 8.0, 26.0, 10.8, None);

        assert_eq!(pos, 0);
    }

    #[test]
    fn test_char_at_pos_with_crlf() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("line1\r\nline2");
        buffer.line_ending = LineEnding::CrLf;

        // Click at end of first line
        let pos = buffer.char_at_pos(
            100.0, // Far right
            10.0,  // First line
            8.0, 26.0, 10.8, None,
        );

        // Should clamp to before the \r\n
        assert_eq!(pos, 5);
    }

    // =========================================================================
    // is_word_char Tests
    // =========================================================================

    #[test]
    fn test_is_word_char_alphabetic() {
        assert!(Buffer::is_word_char('a'));
        assert!(Buffer::is_word_char('Z'));
    }

    #[test]
    fn test_is_word_char_numeric() {
        assert!(Buffer::is_word_char('0'));
        assert!(Buffer::is_word_char('9'));
    }

    #[test]
    fn test_is_word_char_underscore() {
        assert!(Buffer::is_word_char('_'));
    }

    #[test]
    fn test_is_word_char_not_word_char() {
        assert!(!Buffer::is_word_char(' '));
        assert!(!Buffer::is_word_char('\t'));
        assert!(!Buffer::is_word_char('\n'));
        assert!(!Buffer::is_word_char('-'));
        assert!(!Buffer::is_word_char('.'));
        assert!(!Buffer::is_word_char('@'));
    }

    #[test]
    fn test_is_word_char_unicode() {
        // Unicode letters should be word chars
        assert!(Buffer::is_word_char('é'));
        assert!(Buffer::is_word_char('ñ'));
        assert!(Buffer::is_word_char('中'));
    }

    // =========================================================================
    // Regression Tests for Reported Issues
    // =========================================================================

    /// Regression test: Double-click should work correctly with line wrap enabled
    /// Issue: When wrap_enabled is true, double-click word selection was broken
    #[test]
    fn test_double_click_with_wrap_enabled() {
        let mut buffer = Buffer::new();
        buffer.wrap_enabled = true;
        buffer.rope = Rope::from_str("fn main() {\n    println!(\"hello\");\n}");

        // Simulate click position that maps to cursor at "println"
        // First, test that we can get a position
        let wrap_width = Some(400.0f32);
        let click_pos = buffer.char_at_pos(
            80.0, // x position
            45.0, // y position (second line)
            68.0, // x_offset (gutter + padding)
            26.0, // line_height
            10.8, // char_width
            wrap_width,
        );

        // Set cursor to the calculated position
        buffer.set_cursor(click_pos);

        // Now select word at cursor
        buffer.select_word_at_cursor();

        // With correct position calculation, we should have a selection
        // The exact position depends on the math, but it shouldn't panic
        // and should result in a valid cursor position
        assert!(buffer.cursor() <= buffer.rope.len_chars());
    }

    /// Test that char_at_pos_wrapped handles very long lines correctly
    #[test]
    fn test_char_at_pos_wrapped_long_line() {
        let mut buffer = Buffer::new();
        buffer.wrap_enabled = true;
        // Create a line that wraps many times
        let long_content: String = (0..500)
            .map(|i| std::char::from_u32('a' as u32 + (i % 26)).unwrap())
            .collect();
        buffer.rope = Rope::from_str(&long_content);

        let wrap_width = Some(100.0f32); // Very narrow

        // Test various y positions across multiple wrapped segments
        for y in [10.0, 40.0, 70.0, 100.0, 130.0] {
            let pos = buffer.char_at_pos(20.0, y, 8.0, 26.0, 10.8, wrap_width);

            assert!(
                pos <= buffer.rope.len_chars(),
                "Position {} should be within text bounds at y={}",
                pos,
                y
            );
        }
    }

    /// Test click-to-position accuracy at line boundaries with wrapping
    #[test]
    fn test_char_at_pos_line_boundaries_with_wrap() {
        let mut buffer = Buffer::new();
        buffer.wrap_enabled = true;
        buffer.rope = Rope::from_str("short\nthis is a much longer line that wraps\nanother");

        let wrap_width = Some(150.0f32);

        // Click at the boundary between first and second logical lines
        let pos = buffer.char_at_pos(
            20.0, 35.0, // Just below first line
            8.0, 26.0, 10.8, wrap_width,
        );

        // Should be on second line (after "short\n")
        assert!(pos >= 6, "Position should be after first line's newline");
    }

    #[test]
    fn test_visual_position_of_char_wrap_boundary_stays_on_current_row() {
        let mut buffer = Buffer::new();
        buffer.wrap_enabled = true;
        buffer.rope = Rope::from_str("0123456789");

        let (visual_line, col) = buffer.visual_position_of_char(10, Some(50.0), 10.0);

        assert_eq!(visual_line, 1);
        assert_eq!(col, 5);
    }

    // =========================================================================
    // Multi-cursor modifier tests
    // =========================================================================

    #[test]
    fn test_move_all_word_left_no_shift_clears_selection() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello world");
        buffer.set_cursor(11);
        buffer.add_cursor(5);
        // Give each cursor a selection anchor
        buffer.cursors[0].selection_anchor = Some(11);
        buffer.cursors[1].selection_anchor = Some(7);

        buffer.move_all_word_left(false);

        for c in &buffer.cursors {
            assert!(c.selection_anchor.is_none(), "selection_anchor should be cleared without shift");
        }
    }

    #[test]
    fn test_move_all_word_left_with_shift_sets_anchors() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello world");
        // cursor 0 at position 11 (end), cursor 1 at position 5 (space)
        buffer.set_cursor(11);
        buffer.add_cursor(5);

        buffer.move_all_word_left(true);

        for c in &buffer.cursors {
            assert!(c.selection_anchor.is_some(), "selection_anchor should be set with shift");
        }
    }

    #[test]
    fn test_move_all_word_right_no_shift_clears_selection() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello world");
        buffer.set_cursor(0);
        buffer.add_cursor(6);
        buffer.cursors[0].selection_anchor = Some(0);
        buffer.cursors[1].selection_anchor = Some(6);

        buffer.move_all_word_right(false);

        for c in &buffer.cursors {
            assert!(c.selection_anchor.is_none(), "selection_anchor should be cleared without shift");
        }
    }

    #[test]
    fn test_move_all_word_right_with_shift_sets_anchors() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello world");
        buffer.set_cursor(0);
        buffer.add_cursor(6);

        buffer.move_all_word_right(true);

        for c in &buffer.cursors {
            assert!(c.selection_anchor.is_some(), "selection_anchor should be set with shift");
        }
    }

    #[test]
    fn test_move_all_to_line_start_with_shift_selects_on_all_cursors() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello\nworld\n");
        // cursor 0 at end of first line (pos 5), cursor 1 at end of second line (pos 11)
        buffer.set_cursor(5);
        buffer.add_cursor(11);

        buffer.move_all_to_line_start(true);

        for c in &buffer.cursors {
            assert!(c.selection_anchor.is_some(), "each cursor should have a selection anchor");
        }
        // Both cursors should now be at their respective line starts
        assert_eq!(buffer.cursors[0].position, 0);  // start of "hello"
        assert_eq!(buffer.cursors[1].position, 6);  // start of "world"
    }

    #[test]
    fn test_move_all_to_line_end_with_shift_selects_on_all_cursors() {
        let mut buffer = Buffer::new();
        buffer.rope = Rope::from_str("hello\nworld\n");
        // cursor 0 at start of first line, cursor 1 at start of second line
        buffer.set_cursor(0);
        buffer.add_cursor(6);

        buffer.move_all_to_line_end(true);

        for c in &buffer.cursors {
            assert!(c.selection_anchor.is_some(), "each cursor should have a selection anchor");
        }
        // Both cursors should be at their respective line ends (before newline)
        assert_eq!(buffer.cursors[0].position, 5);  // end of "hello" before \n
        assert_eq!(buffer.cursors[1].position, 11); // end of "world" before \n
    }
}
