use std::ops::Range;

const MARK: char = '\u{feff}';

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(String),
    Text(String),
    Numbers(Vec<String>),
    Texts(Vec<String>),
    Tokens(Vec<String>),
    Flag(bool),
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub key: String,
    pub value: Value,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub path: Vec<String>,
    pub fields: Vec<Field>,
    pub span: Range<usize>,
    pub leaf: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub path: Vec<String>,
    pub span: Range<usize>,
}

impl Record {
    pub fn field(&self, key: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.key == key)
    }
}

enum Shape {
    Scalar(Value),
    Numbers(Vec<String>),
    Texts(Vec<String>),
    Tokens(Vec<String>),
    Nested,
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
    path: Vec<String>,
    records: Vec<Record>,
    nodes: Vec<Node>,
}

impl Parser<'_> {
    fn byte(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn skip(&mut self) {
        while self.byte().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    fn expect(&mut self, wanted: u8) -> Option<()> {
        self.skip();

        if self.byte()? != wanted {
            return None;
        }

        self.at += 1;

        Some(())
    }

    fn string(&mut self) -> Option<String> {
        self.skip();

        let start = self.at;

        if self.byte()? != b'"' {
            return None;
        }

        self.at += 1;

        loop {
            match self.byte()? {
                b'\\' => self.at += 2,
                b'"' => {
                    self.at += 1;

                    break;
                }
                _ => self.at += 1,
            }
        }

        let token = self.text.get(start..self.at)?;

        serde_json::from_str::<String>(token).ok()
    }

    fn scalar(&mut self) -> Option<Value> {
        self.skip();

        let start = self.at;

        if self.byte()? == b'"' {
            return self.string().map(Value::Text);
        }

        while self.byte().is_some_and(|byte| !matches!(byte, b',' | b'}' | b']') && !byte.is_ascii_whitespace()) {
            self.at += 1;
        }

        let token = self.text.get(start..self.at)?;

        match token {
            "true" => Some(Value::Flag(true)),
            "false" => Some(Value::Flag(false)),
            "null" => Some(Value::Other),
            _ => token.parse::<f64>().is_ok().then(|| Value::Number(token.to_owned())),
        }
    }

    fn value(&mut self) -> Option<(Shape, Range<usize>)> {
        self.skip();

        let start = self.at;

        let shape = match self.byte()? {
            b'{' => {
                self.object()?;

                Shape::Nested
            }
            b'[' => self.array()?,
            _ => Shape::Scalar(self.scalar()?),
        };

        Some((shape, start..self.at))
    }

    fn object(&mut self) -> Option<()> {
        self.skip();

        let start = self.at;

        self.expect(b'{')?;

        let mut fields: Vec<Field> = Vec::new();
        let mut leaf = true;

        self.skip();

        if self.byte()? == b'}' {
            self.at += 1;
            self.nodes.push(Node { path: self.path.clone(), span: start..self.at });

            return Some(());
        }

        loop {
            let key = self.string()?;

            self.expect(b':')?;
            self.path.push(key.clone());

            let (shape, span) = self.value()?;

            self.path.pop();

            match shape {
                Shape::Scalar(value) => fields.push(Field { key, value, span }),
                Shape::Numbers(items) => fields.push(Field { key, value: Value::Numbers(items), span }),
                Shape::Texts(items) => fields.push(Field { key, value: Value::Texts(items), span }),
                Shape::Tokens(items) => fields.push(Field { key, value: Value::Tokens(items), span }),
                Shape::Nested => leaf = false,
            }

            self.skip();

            match self.byte()? {
                b',' => self.at += 1,
                b'}' => {
                    self.at += 1;

                    break;
                }
                _ => return None,
            }
        }

        self.nodes.push(Node { path: self.path.clone(), span: start..self.at });

        if !fields.is_empty() {
            self.records.push(Record { path: self.path.clone(), fields, span: start..self.at, leaf });
        }

        Some(())
    }

    fn array(&mut self) -> Option<Shape> {
        self.expect(b'[')?;

        let mut items: Vec<String> = Vec::new();
        let mut words: Vec<String> = Vec::new();
        let mut tokens: Vec<String> = Vec::new();
        let mut flat = true;
        let mut index = 0usize;

        self.skip();

        if self.byte()? == b']' {
            self.at += 1;

            return Some(Shape::Numbers(items));
        }

        loop {
            self.path.push(index.to_string());

            let (shape, span) = self.value()?;

            self.path.pop();

            if let Some(token) = self.text.get(span) {
                tokens.push(token.to_owned());
            }

            match shape {
                Shape::Scalar(Value::Number(number)) => items.push(number),
                Shape::Scalar(Value::Text(word)) => words.push(word),
                Shape::Scalar(_) => {}
                _ => flat = false,
            }

            index += 1;
            self.skip();

            match self.byte()? {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;

                    break;
                }
                _ => return None,
            }
        }

        let count = tokens.len();

        Some(match flat {
            false => Shape::Nested,
            true if items.len() == count => Shape::Numbers(items),
            true if words.len() == count => Shape::Texts(words),
            true => Shape::Tokens(tokens),
        })
    }
}

fn parsed(text: &str) -> Option<Parser<'_>> {
    let at = text.strip_prefix(MARK).map_or(0, |_| MARK.len_utf8());
    let mut parser = Parser { text, at, path: Vec::new(), records: Vec::new(), nodes: Vec::new() };

    parser.value()?;

    Some(parser)
}

pub fn records(text: &str) -> Vec<Record> {
    parsed(text).map_or_else(Vec::new, |parser| parser.records)
}

pub fn objects(text: &str) -> Vec<Node> {
    parsed(text).map_or_else(Vec::new, |parser| parser.nodes)
}

pub fn insert(text: &str, span: &Range<usize>, key: &str, value: &str) -> Option<String> {
    let object = text.get(span.clone())?;
    let close = span.start + object.rfind('}')?;
    let body = text.get(span.start..close)?;
    let last = span.start + body.trim_end().len();
    let empty = body.trim_end().ends_with('{');
    let joint = if object.contains("\": ") { ": " } else { ":" };

    let lead = match object.contains('\n') {
        true => {
            let line_start = text.get(..last)?.rfind('\n').map_or(0, |at| at + 1);
            let indent: String = text.get(line_start..last)?.chars().take_while(|glyph| glyph.is_whitespace()).collect();

            format!("\n{indent}")
        }
        false => String::new(),
    };

    let comma = if empty { "" } else { "," };
    let entry = format!("{comma}{lead}{}{joint}{value}", quote(key));

    let head = text.get(..last)?;
    let tail = text.get(last..)?;

    Some(format!("{head}{entry}{tail}"))
}

pub fn replace(text: &str, span: &Range<usize>, value: &str) -> Option<String> {
    let head = text.get(..span.start)?;
    let tail = text.get(span.end..)?;

    Some(format!("{head}{value}{tail}"))
}

pub fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
}

pub fn numbers(text: &str, span: &Range<usize>, items: &[i64]) -> String {
    let tokens: Vec<String> = items.iter().map(i64::to_string).collect();

    list(text, span, &tokens)
}

pub fn texts(text: &str, span: &Range<usize>, items: &[String]) -> String {
    let tokens: Vec<String> = items.iter().map(|item| quote(item)).collect();

    list(text, span, &tokens)
}

pub fn tokens(text: &str, span: &Range<usize>, items: &[String]) -> String {
    list(text, span, items)
}

pub fn tokenize(typed: &str) -> Option<Vec<String>> {
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&format!("[{typed}]")).ok()?;

    parsed
        .iter()
        .map(|item| match item {
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => None,
            scalar => serde_json::to_string(scalar).ok(),
        })
        .collect()
}

fn canonical(original: &str) -> Option<Vec<String>> {
    let parsed: Vec<serde_json::Value> = serde_json::from_str(original).ok()?;

    parsed.iter().map(|item| serde_json::to_string(item).ok()).collect()
}

fn list(text: &str, span: &Range<usize>, items: &[String]) -> String {
    let original = text.get(span.clone()).unwrap_or_default();

    if canonical(original).is_some_and(|held| held == items) {
        return original.to_owned();
    }

    if items.is_empty() {
        return "[]".to_owned();
    }

    if !original.contains('\n') {
        let joint = if original.contains(", ") || !original.contains(',') { ", " } else { "," };

        return format!("[{}]", items.join(joint));
    }

    let line_start = text.get(..span.start).and_then(|head| head.rfind('\n')).map_or(0, |at| at + 1);
    let base: String = text.get(line_start..span.start).unwrap_or_default().chars().take_while(|glyph| glyph.is_whitespace()).collect();

    let inner = original
        .lines()
        .nth(1)
        .map(|line| line.chars().take_while(|glyph| glyph.is_whitespace()).collect::<String>())
        .filter(|indent| indent.len() > base.len())
        .unwrap_or_else(|| format!("{base}  "));

    let rows = original.lines().count().saturating_sub(2);
    let packed = canonical(original).is_some_and(|held| held.len() > rows);

    if packed {
        let joint = if original.contains(", ") { ", " } else { "," };

        return format!("[\n{inner}{}\n{base}]", items.join(joint));
    }

    let listed: Vec<String> = items.iter().map(|item| format!("{inner}{item}")).collect();

    format!("[\n{}\n{base}]", listed.join(",\n"))
}

#[cfg(test)]
mod tests {
    use super::{insert, numbers, objects, records, replace, texts, tokenize, tokens, Value};

    const BOOK: &str = "{\n  \"BonusGroupID\": {\n    \"0\": {\n      \"BonusTarget\": 0,\n      \"BonusTargetID\": [\n        4,\n        5\n      ]\n    }\n  }\n}\n";

    #[test]
    fn a_record_is_found_by_its_path_with_every_field_spanned() {
        let found = records(BOOK);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, vec!["BonusGroupID", "0"]);

        assert!(BOOK[found[0].span.clone()].starts_with('{') && BOOK[found[0].span.clone()].ends_with('}'));

        let target = found[0].field("BonusTarget").expect("the scalar field");
        assert_eq!(&BOOK[target.span.clone()], "0");

        let listed = found[0].field("BonusTargetID").expect("the list field");
        assert_eq!(listed.value, Value::Numbers(vec!["4".into(), "5".into()]));
    }

    #[test]
    fn an_edit_touches_only_its_own_value() {
        let found = records(BOOK);
        let listed = found[0].field("BonusTargetID").expect("the list field");

        let rendered = numbers(BOOK, &listed.span, &[4, 5, 6]);
        let edited = replace(BOOK, &listed.span, &rendered).expect("the span is inside the text");

        // the new list keeps the file's own one-per-line layout and indentation
        assert!(edited.contains("\"BonusTargetID\": [\n        4,\n        5,\n        6\n      ]"));

        let unchanged = replace(BOOK, &listed.span, &numbers(BOOK, &listed.span, &[4, 5])).expect("same span");
        assert_eq!(unchanged, BOOK, "rendering the same list back is byte for byte");
    }

    #[test]
    fn string_lists_and_flags_are_their_own_values() {
        let preset = "{\"slot\":{\"data\":{\"0\":{\"chara\":[\"0\",\"882\"],\"cannon\":\"0\"}}},\"chara\":{\"data\":{\"9\":{\"remove\":true}}}}";
        let found = records(preset);

        let slot = found.iter().find(|record| record.path == ["slot", "data", "0"]).expect("the slot is a record");
        let chara = slot.field("chara").expect("the lineup");
        assert_eq!(chara.value, Value::Texts(vec!["0".into(), "882".into()]));

        // a compact file stays compact when the list grows
        let grown = replace(preset, &chara.span, &texts(preset, &chara.span, &["0".into(), "882".into(), "5".into()])).expect("in bounds");
        assert!(grown.contains("\"chara\":[\"0\",\"882\",\"5\"]"));

        let removed = found.iter().find(|record| record.path == ["chara", "data", "9"]).expect("a removed unit");
        assert_eq!(removed.field("remove").map(|field| &field.value), Some(&Value::Flag(true)));
    }

    #[test]
    fn a_key_is_inserted_in_the_style_of_its_object() {
        let compact = "{\"data\":{\"0\":{\"level\":\"1\"}}}";
        let data = objects(compact).into_iter().find(|node| node.path == ["data"]).expect("the data object");
        let added = insert(compact, &data.span, "7", "{\"level\":\"5\"}").expect("in bounds");
        assert_eq!(added, "{\"data\":{\"0\":{\"level\":\"1\"},\"7\":{\"level\":\"5\"}}}");

        let empty = objects("{\"data\":{}}").into_iter().find(|node| node.path == ["data"]).expect("an empty object");
        assert_eq!(insert("{\"data\":{}}", &empty.span, "0", "1").as_deref(), Some("{\"data\":{\"0\":1}}"));

        let root = objects(BOOK).into_iter().find(|node| node.path == ["BonusGroupID"]).expect("the group object");
        let pretty = insert(BOOK, &root.span, "1", "{}").expect("in bounds");
        assert!(pretty.contains("    },\n    \"1\": {}\n  }"), "the new key lines up with its siblings");
    }

    #[test]
    fn an_object_with_children_keeps_its_own_scalars_as_a_branch() {
        let found = records("{\"MapID\":{\"7\":{\"ContentsType\":1,\"RuleType\":{\"0\":{\"Parameters\":[5]}}}}}");

        let branch = found.iter().find(|record| record.path == ["MapID", "7"]).expect("the map keeps its scalars");
        assert!(!branch.leaf);
        assert_eq!(branch.fields.len(), 1, "the nested rule is not one of its fields");

        assert!(found.iter().any(|record| record.leaf && record.path == ["MapID", "7", "RuleType", "0"]));
    }

    #[test]
    fn a_list_of_mixed_scalars_keeps_its_raw_tokens() {
        let body = "{\"Rule\":{\"Labels\":[\"Headline001\",-1,-1]}}";
        let found = records(body);
        let labels = found[0].field("Labels").expect("the mixed list");

        assert_eq!(labels.value, Value::Tokens(vec!["\"Headline001\"".into(), "-1".into(), "-1".into()]));

        let typed = tokenize("\"Headline001\", \"Headline002\", -1").expect("valid tokens");
        let edited = replace(body, &labels.span, &tokens(body, &labels.span, &typed)).expect("in bounds");
        assert!(edited.contains("[\"Headline001\",\"Headline002\",-1]"));

        assert!(tokenize("[1]").is_none(), "a nested list is not a token");
        assert!(tokenize("bare words").is_none());
    }

    #[test]
    fn a_packed_list_keeps_its_items_on_one_line() {
        let body = "{\"Pop\":{\n  \"CharaID\":[\n    0,1,2\n    ],\n  \"Value\":2\n}}";
        let found = records(body);
        let listed = found[0].field("CharaID").expect("the packed list");

        assert_eq!(numbers(body, &listed.span, &[0, 1, 2]), body[listed.span.clone()], "the same values come back byte for byte");
        assert_eq!(numbers(body, &listed.span, &[0, 1, 2, 3]), "[\n    0,1,2,3\n  ]");
    }

    #[test]
    fn records_inside_lists_are_addressed_by_index() {
        let found = records("{\"Data\": [{\"MapID\": [39001], \"Name\": \"a.preset\"}, {\"MapID\": [39002]}]}");

        assert_eq!(found.iter().map(|record| record.path.join("/")).collect::<Vec<_>>(), vec!["Data/0", "Data/1"]);
    }
}
