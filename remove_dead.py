#!/usr/bin/env python3
import re

dead_methods = [
    'desired_col', 'all_selection_ranges', 'from_file',
    'current_large_file_byte_offset', 'toggle_bookmark',
    'backspace', 'delete_forward',
    'move_left', 'move_right', 'move_up', 'move_down',
    'move_left_sel', 'move_right_sel', 'move_up_sel', 'move_down_sel',
    'move_to_line_start', 'move_to_line_end',
    'get_selected_text', 'copy', 'cut',
    'move_word_left', 'move_word_right',
    'delete_word_left', 'delete_word_right',
    'selection_range',
]

with open('src/editor/buffer.rs', 'r') as f:
    lines = f.readlines()

to_remove = set()

for method in dead_methods:
    pattern = re.compile(r'^\s+pub fn ' + re.escape(method) + r'\(')
    for i, line in enumerate(lines):
        if pattern.match(line):
            m = re.match(r'^\s+pub fn (\w+)\(', line)
            if m and m.group(1) == method:
                doc_start = i
                j = i - 1
                while j >= 0:
                    stripped = lines[j].strip()
                    if stripped.startswith('///') or stripped.startswith('//!'):
                        doc_start = j
                        j -= 1
                    elif stripped == '' and j > 0 and lines[j-1].strip().startswith('///'):
                        doc_start = j
                        j -= 1
                    else:
                        break

                depth = 0
                method_end = i
                for k in range(i, len(lines)):
                    depth += lines[k].count('{') - lines[k].count('}')
                    if depth == 0:
                        method_end = k
                        break

                for k in range(doc_start, method_end + 1):
                    to_remove.add(k)
                if method_end + 1 < len(lines) and lines[method_end + 1].strip() == '':
                    to_remove.add(method_end + 1)

                print(f"  Remove: {method} (lines {doc_start+1}-{method_end+1})")
                break

new_lines = [line for i, line in enumerate(lines) if i not in to_remove]
with open('src/editor/buffer.rs', 'w') as f:
    f.writelines(new_lines)

print(f"\nRemoved {len(to_remove)} lines ({len(dead_methods)} methods)")
