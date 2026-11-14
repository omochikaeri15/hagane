use std::fmt;

const DEPTH_LIMIT: usize = 64;
const MARK: &[u8] = &[0xef, 0xbb, 0xbf];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Null,
    Flag(bool),
    Number(String),
    Text(String),
    List(Vec<Node>),
    Object(Vec<(String, Node)>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed {
    pub at: usize,
}

impl fmt::Display for Malformed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "The file stops being JSON at byte {}.", self.at)
    }
}

impl std::error::Error for Malformed {}

pub fn numeric(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut at = usize::from(bytes.first() == Some(&b'-'));

    let digits = |from: usize| bytes[from.min(bytes.len())..].iter().take_while(|byte| byte.is_ascii_digit()).count();
    let whole = digits(at);

    if whole == 0 || (whole > 1 && bytes[at] == b'0') {
        return false;
    }

    at += whole;

    if bytes.get(at) == Some(&b'.') {
        let fraction = digits(at + 1);

        if fraction == 0 {
            return false;
        }

        at += 1 + fraction;
    }

    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        let signed = usize::from(matches!(bytes.get(at + 1), Some(b'+' | b'-')));
        let exponent = digits(at + 1 + signed);

        if exponent == 0 {
            return false;
        }

        at += 1 + signed + exponent;
    }

    at == bytes.len()
}

pub fn whole(text: &str) -> bool {
    numeric(text) && !text.contains(['.', 'e', 'E'])
}

pub fn plain(raw: &str) -> String {
    if !raw.contains('\\') {
        return raw.to_owned();
    }

    serde_json::from_str::<String>(&format!("\"{raw}\"")).unwrap_or_else(|_| raw.to_owned())
}

fn named(raw: &str, key: &str) -> bool {
    if raw.contains('\\') { plain(raw) == key } else { raw == key }
}

pub fn escaped(text: &str) -> String {
    let quoted = serde_json::to_string(text).unwrap_or_default();

    quoted.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')).map_or_else(String::new, str::to_owned)
}

impl Node {
    pub fn text(content: &str) -> Self {
        Self::Text(escaped(content))
    }

    pub fn get(&self, key: &str) -> Option<&Node> {
        let Self::Object(members) = self else {
            return None;
        };

        members.iter().find(|(name, _)| named(name, key)).map(|(_, node)| node)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Node> {
        let Self::Object(members) = self else {
            return None;
        };

        members.iter_mut().find(|(name, _)| named(name, key)).map(|(_, node)| node)
    }

    pub fn set(&mut self, key: &str, node: Node) {
        let Self::Object(members) = self else {
            return;
        };

        match members.iter_mut().find(|(name, _)| named(name, key)) {
            Some((_, held)) => *held = node,
            None => members.push((escaped(key), node)),
        }
    }

    pub fn remove(&mut self, key: &str) {
        if let Self::Object(members) = self {
            members.retain(|(name, _)| !named(name, key));
        }
    }

    pub fn shown(&self) -> String {
        match self {
            Self::Text(raw) => plain(raw),
            Self::Number(lexeme) => lexeme.clone(),
            other => other.to_string(),
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, Malformed> {
        let mut reader = Reader { bytes, at: 0 };

        reader.blank();

        let node = reader.node(0)?;

        reader.blank();

        if reader.at != bytes.len() {
            return Err(Malformed { at: reader.at });
        }

        Ok(node)
    }
}

impl fmt::Display for Node {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("null"),
            Self::Flag(flag) => write!(formatter, "{flag}"),
            Self::Number(lexeme) => formatter.write_str(lexeme),
            Self::Text(raw) => write!(formatter, "\"{raw}\""),
            Self::List(items) => {
                formatter.write_str("[")?;

                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(",")?;
                    }

                    write!(formatter, "{item}")?;
                }

                formatter.write_str("]")
            }
            Self::Object(members) => {
                formatter.write_str("{")?;

                for (index, (name, node)) in members.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(",")?;
                    }

                    write!(formatter, "\"{name}\":{node}")?;
                }

                formatter.write_str("}")
            }
        }
    }
}

pub fn marked(bytes: &[u8]) -> (bool, &[u8]) {
    bytes.strip_prefix(MARK).map_or((false, bytes), |rest| (true, rest))
}

pub fn mark() -> &'static [u8] {
    MARK
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn fault<T>(&self) -> Result<T, Malformed> {
        Err(Malformed { at: self.at })
    }

    fn blank(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn word(&mut self, word: &[u8], node: Node) -> Result<Node, Malformed> {
        if !self.bytes[self.at..].starts_with(word) {
            return self.fault();
        }

        self.at += word.len();

        Ok(node)
    }

    fn raw_text(&mut self) -> Result<String, Malformed> {
        self.at += 1;

        let start = self.at;

        loop {
            match self.bytes.get(self.at) {
                Some(b'"') => break,
                Some(b'\\') if self.at + 1 < self.bytes.len() => self.at += 2,
                Some(byte) if *byte >= 0x20 => self.at += 1,
                _ => return self.fault(),
            }
        }

        let Ok(raw) = std::str::from_utf8(&self.bytes[start..self.at]) else {
            self.at = start;

            return self.fault();
        };

        if serde_json::from_str::<String>(&format!("\"{raw}\"")).is_err() {
            self.at = start;

            return self.fault();
        }

        self.at += 1;

        Ok(raw.to_owned())
    }

    fn number(&mut self) -> Result<Node, Malformed> {
        let start = self.at;
        let length = self.bytes[start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E'))
            .count();
        let lexeme = std::str::from_utf8(&self.bytes[start..start + length]).unwrap_or_default();

        if !numeric(lexeme) {
            return self.fault();
        }

        self.at += length;

        Ok(Node::Number(lexeme.to_owned()))
    }

    fn node(&mut self, depth: usize) -> Result<Node, Malformed> {
        if depth > DEPTH_LIMIT {
            return self.fault();
        }

        match self.bytes.get(self.at) {
            Some(b'n') => self.word(b"null", Node::Null),
            Some(b't') => self.word(b"true", Node::Flag(true)),
            Some(b'f') => self.word(b"false", Node::Flag(false)),
            Some(b'"') => self.raw_text().map(Node::Text),
            Some(b'[') => self.list(depth),
            Some(b'{') => self.object(depth),
            Some(_) => self.number(),
            None => self.fault(),
        }
    }

    fn list(&mut self, depth: usize) -> Result<Node, Malformed> {
        let mut items = Vec::new();

        self.at += 1;
        self.blank();

        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;

            return Ok(Node::List(items));
        }

        loop {
            self.blank();
            items.push(self.node(depth + 1)?);
            self.blank();

            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => break,
                _ => return self.fault(),
            }
        }

        self.at += 1;

        Ok(Node::List(items))
    }

    fn object(&mut self, depth: usize) -> Result<Node, Malformed> {
        let mut members = Vec::new();

        self.at += 1;
        self.blank();

        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;

            return Ok(Node::Object(members));
        }

        loop {
            self.blank();

            if self.bytes.get(self.at) != Some(&b'"') {
                return self.fault();
            }

            let name = self.raw_text()?;

            self.blank();

            if self.bytes.get(self.at) != Some(&b':') {
                return self.fault();
            }

            self.at += 1;
            self.blank();
            members.push((name, self.node(depth + 1)?));
            self.blank();

            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => break,
                _ => return self.fault(),
            }
        }

        self.at += 1;

        Ok(Node::Object(members))
    }
}

#[cfg(test)]
mod tests {
    use super::{numeric, whole, Node};

    #[test]
    fn a_compact_file_writes_back_byte_for_byte() {
        let source = r#"{"data":[{"name":"塵あ","count":{"value":60},"scale":{"min":{"value":0.5},"max":{"value":1.50}},"list":[1,-2e3,true,null]}]}"#;
        let node = Node::parse(source.as_bytes()).unwrap();

        assert_eq!(node.to_string(), source);
    }

    #[test]
    fn spacing_is_dropped_and_nothing_else() {
        let node = Node::parse(b" { \"id\" : 12 ,\n \"k\" : [ ] } ").unwrap();

        assert_eq!(node.to_string(), r#"{"id":12,"k":[]}"#);
    }

    #[test]
    fn numbers_follow_the_json_grammar() {
        for good in ["0", "-0", "12", "-3.5", "1e9", "2.5E-3"] {
            assert!(numeric(good), "{good}");
        }

        for bad in ["", "-", "01", "1.", ".5", "+1", "1e", "0x10", "1 "] {
            assert!(!numeric(bad), "{bad}");
        }

        assert!(whole("-12"));
        assert!(!whole("1.0"));
    }

    #[test]
    fn broken_input_names_where_it_broke() {
        assert_eq!(Node::parse(b"{\"a\":}").unwrap_err().at, 5);
        assert!(Node::parse(b"{\"a\":1} x").is_err());
        assert!(Node::parse(b"").is_err());
    }

    #[test]
    fn a_key_is_found_through_its_escapes() {
        let mut node = Node::parse(br#"{"x":{"value":1}}"#).unwrap();

        assert!(node.get("x").is_some());

        node.set("x", Node::Number("2".to_owned()));
        assert_eq!(node.to_string(), r#"{"x":2}"#);
    }
}
