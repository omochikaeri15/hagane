use md5::{Digest, Md5};

pub struct Terms {
    pub intro: [&'static str; 2],
    pub facts: &'static [&'static str],
    pub closing: &'static str,
}

impl Terms {
    pub fn text(&self, bullet: &str) -> String {
        let facts: Vec<String> = self.facts.iter().map(|fact| format!("{bullet}{fact}")).collect();

        format!("{}\n\n{}\n\n{}", self.intro.join("\n"), facts.join("\n"), self.closing)
    }

    pub fn agreement(&self) -> String {
        format!("{:x}", Md5::digest(self.text("").as_bytes()))
    }
}
