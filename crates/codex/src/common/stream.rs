use std::borrow::Cow;
use std::ops::Range;

const DELIMITER: char = ',';

const ENDINGS: [u8; 2] = [b'\r', BREAK];

const BLANKS: [u8; 6] = [b' ', b'\t', b'\n', 0x0b, 0x0c, b'\r'];

const COUNT_CAP: usize = 1 << 16;

const BREAK: u8 = b'\n';

const SPAN: usize = 64;

#[derive(Clone, Debug, Default)]
pub struct Reader<'a> {
    rest: &'a str,
    cells: Vec<&'a str>,
    values: Vec<Cow<'a, str>>,
}

impl<'a> Reader<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { rest: text, cells: Vec::new(), values: Vec::new() }
    }

    pub fn line(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() { return None; }

        let (line, rest) = self.rest.split_once('\n').unwrap_or((self.rest, ""));
        self.rest = rest;

        Some(&line[..kept(line.as_bytes())])
    }

    pub fn row(&mut self) {
        let Some(line) = self.line() else { return };

        self.cells.clear();
        self.values.clear();

        if line.is_empty() { return; }

        for field in line.strip_suffix(DELIMITER).unwrap_or(line).split(DELIMITER) {
            self.cells.push(field);
            self.values.push(scalar(field));
        }
    }

    pub fn cells(&self) -> &[&'a str] {
        &self.cells
    }

    pub fn scalars(&self) -> Vec<&str> {
        self.values.iter().map(Cow::as_ref).collect()
    }

    pub fn value(&self, index: usize) -> i32 {
        self.values.get(index).and_then(|text| text.parse().ok()).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor<'a> {
    rest: &'a [u8],
    row: &'a [u8],
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes, row: &[] }
    }

    pub(crate) fn line(&mut self) -> Option<&'a [u8]> {
        next_line(&mut self.rest)
    }

    pub(crate) fn row(&mut self) {
        self.skip(1);
    }

    pub(crate) fn skip(&mut self, rows: usize) {
        if rows > 1
            && let Some(at) = nth_break(self.rest, rows - 2)
            && let Some(after) = self.rest.get(at + 1..).filter(|after| !after.is_empty())
        {
            self.rest = after;

            if let Some(line) = next_line(&mut self.rest) {
                self.row = line;
            }

            return;
        }

        for _ in 0..rows {
            let Some(line) = next_line(&mut self.rest) else { return };

            self.row = line;
        }
    }

    pub(crate) fn value(&self, index: usize) -> i32 {
        if self.row.is_empty() { return 0; }

        self.row
            .strip_suffix(&[DELIMITER as u8])
            .unwrap_or(self.row)
            .split(|&byte| byte == DELIMITER as u8)
            .nth(index)
            .map_or(0, number)
    }
}

fn nth_break(bytes: &[u8], mut nth: usize) -> Option<usize> {
    let mut start = 0;

    for chunk in bytes.chunks(SPAN) {
        let count = usize::from(chunk.iter().fold(0u8, |count, &byte| count.wrapping_add(u8::from(byte == BREAK))));

        if nth < count {
            return chunk.iter().enumerate().filter(|(_, byte)| **byte == BREAK).nth(nth).map(|(at, _)| start + at);
        }

        nth -= count;
        start += chunk.len();
    }

    None
}

fn next_line<'a>(rest: &mut &'a [u8]) -> Option<&'a [u8]> {
    if rest.is_empty() { return None; }

    let (line, after) = rest.iter().position(|&byte| byte == BREAK).map_or((*rest, &[][..]), |at| (&rest[..at], &rest[at + 1..]));
    *rest = after;

    Some(&line[..kept(line)])
}

fn kept(line: &[u8]) -> usize {
    let trimmed = line.iter().rposition(|byte| !ENDINGS.contains(byte)).map_or(0, |at| at + 1);

    if trimmed == 0 { line.len() } else { trimmed }
}

pub fn cell_value(text: &str) -> i32 {
    number(text.as_bytes())
}

pub fn declared_rows(count: i32) -> Option<usize> {
    usize::try_from(count).ok().filter(|count| *count <= COUNT_CAP)
}

fn scalar(cell: &str) -> Cow<'_, str> {
    let Some(span) = leading(cell.as_bytes()) else { return Cow::Borrowed("") };
    let read = &cell[span];

    if read.parse::<i32>().is_ok() { return Cow::Borrowed(read); }

    Cow::Owned(saturated(read.as_bytes()).to_string())
}

fn number(cell: &[u8]) -> i32 {
    leading(cell).map_or(0, |span| saturated(&cell[span]))
}

fn leading(cell: &[u8]) -> Option<Range<usize>> {
    let start = cell.iter().position(|byte| !BLANKS.contains(byte)).unwrap_or(cell.len());
    let signed = start + usize::from(matches!(cell.get(start), Some(b'+' | b'-')));
    let digits = cell[signed..].iter().take_while(|byte| byte.is_ascii_digit()).count();

    (digits > 0).then_some(start..signed + digits)
}

fn saturated(read: &[u8]) -> i32 {
    let (negative, digits) = match read.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, read),
    };
    let bound = if negative { i64::MIN } else { i64::MAX };
    let mut total: i64 = 0;

    for &digit in digits {
        let step = i64::from(digit - b'0');
        let shifted = total.checked_mul(10).and_then(|tens| if negative { tens.checked_sub(step) } else { tens.checked_add(step) });

        let Some(next) = shifted else { return bound as i32 };

        total = next;
    }

    total as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_carry_the_blank_ones_and_stop_at_the_end() {
        let mut reader = Reader::new("first\r\n\nlast");

        assert_eq!(reader.line(), Some("first"));
        assert_eq!(reader.line(), Some(""));
        assert_eq!(reader.line(), Some("last"));
        assert_eq!(reader.line(), None);

        assert_eq!(Reader::new("").line(), None);
        assert_eq!(Reader::new("\n").line(), Some(""));
        assert_eq!(Reader::new("\r").line(), Some("\r"));
    }

    #[test]
    fn a_trailing_delimiter_adds_no_cell() {
        let mut reader = Reader::new("1,2,\n1,2\n,\n\n");

        reader.row();
        assert_eq!(reader.cells(), ["1", "2"]);

        reader.row();
        assert_eq!(reader.cells(), ["1", "2"]);

        reader.row();
        assert_eq!(reader.cells(), [""]);

        reader.row();
        assert_eq!(reader.cells(), [] as [&str; 0]);
    }

    #[test]
    fn a_row_past_the_end_leaves_the_last_one_in_place() {
        let mut reader = Reader::new("7,8");

        reader.row();
        assert_eq!(reader.value(0), 7);

        reader.row();
        assert_eq!(reader.value(0), 7);
        assert_eq!(reader.value(1), 8);
        assert_eq!(reader.value(2), 0);
    }

    #[test]
    fn a_cell_contributes_the_number_in_front_of_its_text() {
        let mut reader = Reader::new(" -12abc,x,3.75,+4,,99999999999");

        reader.row();

        assert_eq!(reader.value(0), -12);
        assert_eq!(reader.value(1), 0);
        assert_eq!(reader.value(2), 3);
        assert_eq!(reader.value(3), 4);
        assert_eq!(reader.value(4), 0);
        assert_eq!(reader.value(5), 99_999_999_999_i64 as i32);

        assert_eq!(cell_value(" 42 tail"), 42);
        assert_eq!(cell_value("tail"), 0);
    }

    #[test]
    fn a_cursor_reads_the_values_a_reader_does() {
        // the cursor only skips the work, so every value it reports has to match the reader's
        let text = "head\r\n7,-3,x\n\n\r\n 12abc, +4 ,99999999999,\n,,5\n-9223372036854775809,1";
        let mut reader = Reader::new(text);
        let mut cursor = Cursor::new(text.as_bytes());

        assert_eq!(reader.line().map(str::as_bytes), cursor.line());

        for _ in 0..8 {
            reader.row();
            cursor.row();

            for index in 0..4 {
                assert_eq!(reader.value(index), cursor.value(index));
            }
        }
    }

    #[test]
    fn skipping_lands_where_reading_row_by_row_does() {
        for text in ["a\n1\n2\r\n\n3,4\n5", "a\n1\n2\n3,4\n\n"] {
            for rows in 0..9 {
                let mut reader = Reader::new(text);
                let mut cursor = Cursor::new(text.as_bytes());

                (0..rows).for_each(|_| reader.row());
                cursor.skip(rows);

                assert_eq!((reader.value(0), reader.value(1)), (cursor.value(0), cursor.value(1)), "{text:?} after {rows} rows");

                reader.row();
                cursor.row();

                assert_eq!(reader.value(0), cursor.value(0), "{text:?} one row past {rows}");
            }
        }
    }

    #[test]
    fn a_count_the_engine_aborts_on_is_refused() {
        assert_eq!(declared_rows(0), Some(0));
        assert_eq!(declared_rows(12), Some(12));
        assert_eq!(declared_rows(-1), None);
        assert_eq!(declared_rows(i32::MAX), None);
    }
}
