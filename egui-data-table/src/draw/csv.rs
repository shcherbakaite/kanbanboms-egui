#![allow(unused)]
//! CSV parsing and writing for clipboard operations.
//! RFC 4180 style: comma separator, quoted fields for values containing comma/newline/quote.

use std::ops::Range;

/// Parse CSV data. Handles quoted fields and escaped quotes.
pub struct ParsedCsv {
    data: String,
    cell_spans: Vec<Range<u32>>,
    row_offsets: Vec<u32>,
}

impl ParsedCsv {
    pub fn parse(data: &str) -> Self {
        let mut s = Self {
            data: String::new(),
            cell_spans: Vec::new(),
            row_offsets: Vec::new(),
        };

        let mut chars = data.chars().peekable();
        s.row_offsets.push(0);

        while chars.peek().is_some() {
            let cell_start = s.data.len() as u32;

            match chars.peek().copied() {
                Some('"') => {
                    chars.next(); // consume opening quote
                    loop {
                        match chars.next() {
                            None => break,
                            Some('"') => {
                                if chars.peek() == Some(&'"') {
                                    chars.next();
                                    s.data.push('"');
                                } else {
                                    break;
                                }
                            }
                            Some(c) => s.data.push(c),
                        }
                    }
                }
                _ => {
                    while let Some(&c) = chars.peek() {
                        if c == ',' || c == '\n' || c == '\r' {
                            break;
                        }
                        chars.next();
                        if c == '\r' {
                            continue;
                        }
                        s.data.push(c);
                    }
                }
            }

            s.cell_spans.push(cell_start..s.data.len() as u32);

            match chars.peek().copied() {
                Some(',') => {
                    chars.next();
                }
                Some('\n') => {
                    chars.next();
                    s.row_offsets.push(s.cell_spans.len() as u32);
                }
                Some('\r') => {
                    chars.next();
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    s.row_offsets.push(s.cell_spans.len() as u32);
                }
                None => break,
                _ => {}
            }
        }

        if *s.row_offsets.last().unwrap_or(&0) != s.cell_spans.len() as u32 {
            s.row_offsets.push(s.cell_spans.len() as u32);
        }

        s.data.shrink_to_fit();
        s.cell_spans.shrink_to_fit();
        s.row_offsets.shrink_to_fit();

        s
    }

    pub fn calc_table_width(&self) -> usize {
        self.row_offsets
            .windows(2)
            .map(|range| range[1] - range[0])
            .max()
            .unwrap_or(0) as usize
    }

    pub fn num_columns_at(&self, row: usize) -> usize {
        if row >= self.row_offsets.len().saturating_sub(1) {
            return 0;
        }
        let start = self.row_offsets[row] as usize;
        let end = self.row_offsets[row + 1] as usize;
        end - start
    }

    pub fn num_rows(&self) -> usize {
        self.row_offsets.len().saturating_sub(1)
    }

    pub fn get_cell(&self, row: usize, column: usize) -> Option<&str> {
        let row_offset = *self.row_offsets.get(row)? as usize;
        let cell_span = self.cell_spans.get(row_offset + column)?;
        Some(&self.data[cell_span.start as usize..cell_span.end as usize])
    }

    pub fn iter_rows(
        &self,
    ) -> impl Iterator<Item = (usize, impl Iterator<Item = (usize, &str)> + '_)> + '_ {
        self.row_offsets.windows(2).enumerate().map(move |(row, range)| {
            let (start, end) = (range[0] as usize, range[1] as usize);
            let row_iter = (start..end).map(move |cell_offset| {
                let cell_span = self.cell_spans.get(cell_offset).unwrap();
                (
                    cell_offset - start,
                    &self.data[cell_span.start as usize..cell_span.end as usize],
                )
            });
            (row, row_iter)
        })
    }
}

/// Write a CSV field, quoting if it contains comma, newline, or quote.
pub fn write_csv_field(buf: &mut String, item: &str) {
    let needs_quotes = item.is_empty()
        || item.contains(',')
        || item.contains('\n')
        || item.contains('\r')
        || item.contains('"');
    if needs_quotes {
        buf.push('"');
        for c in item.chars() {
            match c {
                '"' => buf.push_str("\"\""),
                _ => buf.push(c),
            }
        }
        buf.push('"');
    } else {
        buf.push_str(item);
    }
}

pub fn write_csv_comma(buf: &mut String) {
    buf.push(',');
}

pub fn write_csv_newline(buf: &mut String) {
    buf.push('\n');
}

/// Heuristic: treat as CSV if data contains comma and (has no tabs, or has more commas than tabs).
fn looks_like_csv(data: &str) -> bool {
    if !data.contains(',') {
        return false;
    }
    let tab_count = data.matches('\t').count();
    let comma_count = data.matches(',').count();
    tab_count == 0 || comma_count > tab_count
}

/// Parse clipboard content as either CSV or TSV.
pub fn parse_clipboard(contents: &str) -> ParsedClipboard {
    if looks_like_csv(contents) {
        ParsedClipboard::Csv(ParsedCsv::parse(contents))
    } else {
        ParsedClipboard::Tsv(super::tsv::ParsedTsv::parse(contents))
    }
}

pub enum ParsedClipboard {
    Csv(ParsedCsv),
    Tsv(super::tsv::ParsedTsv),
}

impl ParsedClipboard {
    pub fn calc_table_width(&self) -> usize {
        match self {
            Self::Csv(c) => c.calc_table_width(),
            Self::Tsv(t) => t.calc_table_width(),
        }
    }

    pub fn num_rows(&self) -> usize {
        match self {
            Self::Csv(c) => c.num_rows(),
            Self::Tsv(t) => t.num_rows(),
        }
    }

    pub fn num_columns_at(&self, row: usize) -> usize {
        match self {
            Self::Csv(c) => c.num_columns_at(row),
            Self::Tsv(t) => t.num_columns_at(row),
        }
    }

    pub fn get_cell(&self, row: usize, column: usize) -> Option<&str> {
        match self {
            Self::Csv(c) => c.get_cell(row, column),
            Self::Tsv(t) => t.get_cell(row, column),
        }
    }
}
