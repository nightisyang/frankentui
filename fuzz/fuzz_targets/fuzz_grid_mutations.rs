#![no_main]

use frankenterm_core::{Action, Cursor, Grid, Parser, Scrollback};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Use first two bytes to derive grid dimensions (3..80 x 3..40).
    if data.len() < 2 {
        return;
    }
    let cols = (data[0] % 78).max(3) as u16 + 3; // 3..80
    let rows = (data[1] % 38).max(3) as u16 + 3; // 3..40
    let payload = &data[2..];

    let mut parser = Parser::new();
    let mut grid = Grid::new(cols, rows);
    let mut cursor = Cursor::new(cols, rows);
    let mut scrollback = Scrollback::new(512);

    let actions = parser.feed(payload);
    for action in actions {
        apply_action(action, &mut grid, &mut cursor, &mut scrollback, cols, rows);
    }

    // Post-conditions that must always hold:
    assert_eq!(grid.cols(), cols, "Grid cols changed");
    assert_eq!(grid.rows(), rows, "Grid rows changed");
    assert!(cursor.row < rows, "cursor.row OOB");
    assert!(
        cursor.col < cols || cursor.pending_wrap,
        "cursor.col OOB without pending_wrap"
    );
    assert!(
        cursor.scroll_top() < cursor.scroll_bottom(),
        "Invalid scroll region"
    );
    assert!(cursor.scroll_bottom() <= rows, "scroll_bottom > rows");

    // All cells accessible.
    for r in 0..rows {
        for c in 0..cols {
            let _ = grid.cell(r, c).expect("cell not accessible");
        }
    }
});

fn apply_action(
    action: Action,
    grid: &mut Grid,
    cursor: &mut Cursor,
    scrollback: &mut Scrollback,
    cols: u16,
    rows: u16,
) {
    match action {
        Action::Print(ch) => {
            if cursor.pending_wrap {
                cursor.col = 0;
                if cursor.row + 1 >= cursor.scroll_bottom() {
                    grid.scroll_up_into(
                        cursor.scroll_top(),
                        cursor.scroll_bottom(),
                        1,
                        scrollback,
                    );
                } else if cursor.row + 1 < rows {
                    cursor.row += 1;
                }
                cursor.pending_wrap = false;
            }
            if let Some(cell) = grid.cell_mut(cursor.row, cursor.col) {
                cell.set_content(ch, 1);
                cell.attrs = cursor.attrs;
            }
            if cursor.col + 1 >= cols {
                cursor.pending_wrap = true;
            } else {
                cursor.col += 1;
                cursor.pending_wrap = false;
            }
        }
        Action::Newline | Action::Index => {
            if cursor.row + 1 >= cursor.scroll_bottom() {
                grid.scroll_up_into(
                    cursor.scroll_top(),
                    cursor.scroll_bottom(),
                    1,
                    scrollback,
                );
            } else if cursor.row + 1 < rows {
                cursor.row += 1;
            }
            cursor.pending_wrap = false;
        }
        Action::CarriageReturn => cursor.carriage_return(),
        Action::Tab => {
            cursor.col = cursor.next_tab_stop(cols);
            cursor.pending_wrap = false;
        }
        Action::Backspace => cursor.move_left(1),
        Action::Bell => {}
        Action::CursorUp(n) => cursor.move_up(n),
        Action::CursorDown(n) => cursor.move_down(n, rows),
        Action::CursorRight(n) => cursor.move_right(n, cols),
        Action::CursorLeft(n) => cursor.move_left(n),
        Action::CursorNextLine(n) => {
            cursor.move_down(n, rows);
            cursor.carriage_return();
        }
        Action::CursorPrevLine(n) => {
            cursor.move_up(n);
            cursor.carriage_return();
        }
        Action::CursorColumn(col) => cursor.move_to(cursor.row, col, rows, cols),
        Action::CursorRow(row) => cursor.move_to(row, cursor.col, rows, cols),
        Action::SetScrollRegion { top, bottom } => {
            let bottom = if bottom == 0 {
                rows
            } else {
                bottom.min(rows)
            };
            cursor.set_scroll_region(top, bottom, rows);
            cursor.move_to(0, 0, rows, cols);
            cursor.pending_wrap = false;
        }
        Action::ScrollUp(count) => {
            grid.scroll_up_into(
                cursor.scroll_top(),
                cursor.scroll_bottom(),
                count,
                scrollback,
            );
            cursor.pending_wrap = false;
        }
        Action::ScrollDown(count) => {
            grid.scroll_down(cursor.scroll_top(), cursor.scroll_bottom(), count);
            cursor.pending_wrap = false;
        }
        Action::InsertLines(count) => {
            grid.insert_lines(
                cursor.row,
                count,
                cursor.scroll_top(),
                cursor.scroll_bottom(),
            );
            cursor.pending_wrap = false;
        }
        Action::DeleteLines(count) => {
            grid.delete_lines(
                cursor.row,
                count,
                cursor.scroll_top(),
                cursor.scroll_bottom(),
            );
            cursor.pending_wrap = false;
        }
        Action::InsertChars(count) => {
            grid.insert_chars(cursor.row, cursor.col, count, cursor.attrs.bg);
            cursor.pending_wrap = false;
        }
        Action::DeleteChars(count) => {
            grid.delete_chars(cursor.row, cursor.col, count, cursor.attrs.bg);
            cursor.pending_wrap = false;
        }
        Action::CursorPosition { row, col } => cursor.move_to(row, col, rows, cols),
        Action::EraseInDisplay(mode) => {
            let bg = cursor.attrs.bg;
            match mode {
                0 => grid.erase_below(cursor.row, cursor.col, bg),
                1 => grid.erase_above(cursor.row, cursor.col, bg),
                2 => grid.erase_all(bg),
                _ => {}
            }
        }
        Action::EraseInLine(mode) => {
            let bg = cursor.attrs.bg;
            match mode {
                0 => grid.erase_line_right(cursor.row, cursor.col, bg),
                1 => grid.erase_line_left(cursor.row, cursor.col, bg),
                2 => grid.erase_line(cursor.row, bg),
                _ => {}
            }
        }
        Action::Sgr(params) => cursor.attrs.apply_sgr_params(&params),
        Action::ReverseIndex => {
            if cursor.row <= cursor.scroll_top() {
                grid.scroll_down(cursor.scroll_top(), cursor.scroll_bottom(), 1);
            } else {
                cursor.move_up(1);
            }
        }
        Action::NextLine => {
            cursor.carriage_return();
            if cursor.row + 1 >= cursor.scroll_bottom() {
                grid.scroll_up_into(
                    cursor.scroll_top(),
                    cursor.scroll_bottom(),
                    1,
                    scrollback,
                );
            } else if cursor.row + 1 < rows {
                cursor.row += 1;
            }
            cursor.pending_wrap = false;
        }
        Action::FullReset => {
            *grid = Grid::new(cols, rows);
            *cursor = Cursor::new(cols, rows);
            *scrollback = Scrollback::new(512);
        }
        // These actions don't mutate grid state in this harness.
        Action::DecSet(_)
        | Action::DecRst(_)
        | Action::AnsiSet(_)
        | Action::AnsiRst(_)
        | Action::SaveCursor
        | Action::RestoreCursor
        | Action::SetTitle(_)
        | Action::HyperlinkStart(_)
        | Action::HyperlinkEnd
        | Action::Escape(_) => {}
    }
}
