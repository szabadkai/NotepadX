#!/usr/bin/env python3
"""Mechanical field-access migration for Renderer sub-struct extraction."""
import re

def migrate_renderer():
    with open('src/renderer/mod.rs', 'r') as f:
        content = f.read()

    # === TextEngine: self.X → self.text.X ===
    # Do text_renderer first (longer match) to avoid conflict with text.text.text_renderer
    content = content.replace('self.text_renderer', 'self.text.text_renderer')
    content = content.replace('self.font_system', 'self.text.font_system')
    content = content.replace('self.swash_cache', 'self.text.swash_cache')
    content = content.replace('self.atlas', 'self.text.atlas')
    content = content.replace('self.viewport', 'self.text.viewport')
    # self.cache but NOT self.cached_text_hash or self.cached_spans
    content = re.sub(r'self\.cache(?!d_)', 'self.text.cache', content)

    # === TabBarState: self.tab_X → self.tabs.X ===
    content = content.replace('self.tab_bar_buffer', 'self.tabs.buffer')
    content = content.replace('self.tab_positions', 'self.tabs.positions')
    content = content.replace('self.tab_scroll_offset', 'self.tabs.scroll_offset')
    content = content.replace('self.tab_scroll_max', 'self.tabs.scroll_max')
    content = content.replace('self.tab_overflow', 'self.tabs.overflow')
    content = content.replace('self.tab_arrow_left_buffer', 'self.tabs.arrow_left_buffer')
    content = content.replace('self.tab_arrow_right_buffer', 'self.tabs.arrow_right_buffer')
    content = content.replace('self.tab_all_btn_buffer', 'self.tabs.all_btn_buffer')
    content = content.replace('self.tab_drag_indicator_x', 'self.tabs.drag_indicator_x')

    # === SnackbarRenderState: self.snackbar_X → self.snackbar.X ===
    # Order: longer matches first
    content = content.replace('self.snackbar_dismiss_forever_bounds', 'self.snackbar.dismiss_forever_bounds')
    content = content.replace('self.snackbar_dismiss_bounds', 'self.snackbar.dismiss_bounds')
    content = content.replace('self.snackbar_next_tip_bounds', 'self.snackbar.next_tip_bounds')
    content = content.replace('self.snackbar_bounds', 'self.snackbar.bounds')
    content = content.replace('self.snackbar_buffer', 'self.snackbar.buffer')
    content = content.replace('self.hovered_snackbar_button', 'self.snackbar.hovered_button')

    with open('src/renderer/mod.rs', 'w') as f:
        f.write(content)
    print(f"renderer/mod.rs: done")


def migrate_main():
    with open('src/main.rs', 'r') as f:
        content = f.read()

    # === TabBarState: renderer.tab_X → renderer.tabs.X ===
    content = content.replace('renderer.tab_positions', 'renderer.tabs.positions')
    content = content.replace('renderer.tab_scroll_offset', 'renderer.tabs.scroll_offset')
    content = content.replace('renderer.tab_scroll_max', 'renderer.tabs.scroll_max')
    content = content.replace('renderer.tab_overflow', 'renderer.tabs.overflow')
    content = content.replace('renderer.tab_drag_indicator_x', 'renderer.tabs.drag_indicator_x')

    # === SnackbarRenderState: renderer.snackbar_X → renderer.snackbar.X ===
    content = content.replace('renderer.snackbar_dismiss_forever_bounds', 'renderer.snackbar.dismiss_forever_bounds')
    content = content.replace('renderer.snackbar_dismiss_bounds', 'renderer.snackbar.dismiss_bounds')
    content = content.replace('renderer.snackbar_next_tip_bounds', 'renderer.snackbar.next_tip_bounds')
    content = content.replace('renderer.snackbar_bounds', 'renderer.snackbar.bounds')
    content = content.replace('renderer.hovered_snackbar_button', 'renderer.snackbar.hovered_button')

    with open('src/main.rs', 'w') as f:
        f.write(content)
    print(f"main.rs: done")


if __name__ == '__main__':
    migrate_renderer()
    migrate_main()
    print("Migration complete. Run `cargo build` to verify.")
