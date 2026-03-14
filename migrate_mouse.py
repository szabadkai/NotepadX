#!/usr/bin/env python3
"""Mechanical field-access migration for App MouseState extraction."""

def migrate():
    with open('src/main.rs', 'r') as f:
        content = f.read()

    # Replace self.field → self.mouse.field for the 7 mouse state fields
    # Order: longer matches first to avoid partial replacement
    content = content.replace('self.block_drag_anchor', 'self.mouse.block_drag_anchor')
    content = content.replace('self.scrollbar_drag', 'self.mouse.scrollbar_drag')
    content = content.replace('self.suppress_drag', 'self.mouse.suppress_drag')
    content = content.replace('self.tab_drag', 'self.mouse.tab_drag')
    # Note: click_count as a field is ONLY in MouseState now;
    # the parameter `click_count` in handle_mouse_click(click_count) is different
    # self.click_count should not exist anymore (was removed from App struct),
    # but any remaining self.click_count references need to be self.mouse.click_count

    with open('src/main.rs', 'w') as f:
        f.write(content)
    print("main.rs: done")

if __name__ == '__main__':
    migrate()
    print("Migration complete.")
