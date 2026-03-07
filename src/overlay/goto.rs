/// Goto Line state
pub fn goto_line(input: &str) -> Option<usize> {
    input.trim().parse::<usize>().ok().map(|n| n.saturating_sub(1))
}
