
use nano_crl2::core::syntax::SourceRange;

use tower_lsp::lsp_types::{Position, Range};

pub fn lsp_range_to_source_range(range: Range) -> SourceRange {
    SourceRange::new(
        range.start.line,
        range.start.character,
        range.end.line,
        range.end.character,
    )
}

pub fn source_range_to_lsp_range(range: SourceRange) -> Range {
    Range::new(
        Position::new(range.get_start_line(), range.get_start_char()),
        Position::new(range.get_end_line(), range.get_end_char()),
    )
}
