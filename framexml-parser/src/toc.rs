use log::warn;
use std::collections::HashMap;

#[derive(Debug)]
pub struct TocFile {
    pub directives: HashMap<String, String>,
    pub files: Vec<String>,
    pub comments: Vec<String>,
}

impl TocFile {
    pub fn parse_file<R: std::io::BufRead>(reader: R) -> Result<Self, std::io::Error> {
        let mut directives = HashMap::new();
        let mut files = Vec::new();
        let mut comments = Vec::new();

        for line_res in reader.lines() {
            let line = line_res?;

            if line.starts_with("## ") && line.contains(":") {
                // what about whitespaces prepending?
                // there are other oddities possible, such as comments having ":", that will be missed by the comments vec then
                Self::parse_directive(&line, &mut directives);
            } else if line.contains("#") {
                // probably not the most robust comment handling.
                let (file_raw, comment_raw) = line.split_once("#").unwrap();
                let comment = comment_raw.trim().to_string();
                let file = file_raw.trim().to_string();

                if !comment.is_empty() {
                    comments.push(comment);
                }

                if !file.is_empty() {
                    files.push(file);
                }
            } else {
                let file = line.trim().to_string();
                if !file.is_empty() {
                    files.push(file);
                }
            }
        }

        Ok(TocFile {
            directives,
            files,
            comments,
        })
    }

    fn parse_directive(line: &str, directives: &mut HashMap<String, String>) {
        let line = line.trim_start_matches("## ");
        let Some((key, value)) = line.split_once(":") else {
            warn!("Invalid directive format: {}", line);
            return;
        };

        directives.insert(key.trim().to_string(), value.trim().to_string());
    }
}
