use std::collections::HashMap;

/// Split a line (without its ending) into the first token and the rest.
fn first_token(line: &str) -> (&str, &str) {
    let trimmed = line.trim_start();
    match trimmed.find(|c: char| c.is_whitespace()) {
        Some(i) => (&trimmed[..i], trimmed[i..].trim()),
        None => (trimmed, ""),
    }
}

/// Rewrite one line's body when its first token matches, keeping the
/// original line-ending bytes untouched.
fn rewrite_lines(text: &str, mut rewrite: impl FnMut(&str, &str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    for piece in text.split_inclusive('\n') {
        let (body, ending) = match piece.strip_suffix('\n') {
            Some(without_lf) => match without_lf.strip_suffix('\r') {
                Some(without_crlf) => (without_crlf, "\r\n"),
                None => (without_lf, "\n"),
            },
            None => (piece, ""),
        };
        let (token, rest) = first_token(body);
        match rewrite(token, rest) {
            Some(new_body) => {
                out.push_str(&new_body);
                out.push_str(ending);
            }
            None => out.push_str(piece),
        }
    }
    out
}

pub fn rewrite_obj(obj: &str, mtl_file_name: &str, renames: &HashMap<String, String>) -> String {
    rewrite_lines(obj, |token, rest| match token {
        "mtllib" => Some(format!("mtllib {mtl_file_name}")),
        "usemtl" => renames.get(rest).map(|new| format!("usemtl {new}")),
        _ => None,
    })
}

pub fn rewrite_mtl(mtl: &str, renames: &HashMap<String, String>) -> String {
    rewrite_lines(mtl, |token, rest| match token {
        "newmtl" => renames.get(rest).map(|new| format!("newmtl {new}")),
        _ => None,
    })
}

pub fn mtllib_of(obj: &str) -> Option<String> {
    for line in obj.lines() {
        let (token, rest) = first_token(line);
        if token == "mtllib" {
            return Some(rest.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn renames_usemtl_and_mtllib() {
        let r = HashMap::from([("Red".into(), "Seat".into())]);
        let out = rewrite_obj("mtllib cube_2mat.mtl\nusemtl Red\nf 1 2 3\nusemtl Blue\n", "Chair.mtl", &r);
        assert_eq!(out, "mtllib Chair.mtl\nusemtl Seat\nf 1 2 3\nusemtl Blue\n");
    }

    #[test]
    fn renames_newmtl() {
        let r = HashMap::from([("Red".into(), "Seat".into())]);
        assert_eq!(rewrite_mtl("newmtl Red\nKd 1 0 0\n", &r), "newmtl Seat\nKd 1 0 0\n");
    }

    #[test]
    fn keeps_crlf_lines() {
        let r = HashMap::from([("Red".into(), "Seat".into())]);
        let out = rewrite_obj("mtllib a.mtl\r\nusemtl Red\r\nf 1 2 3\r\n", "B.mtl", &r);
        assert_eq!(out, "mtllib B.mtl\r\nusemtl Seat\r\nf 1 2 3\r\n");
    }

    #[test]
    fn mtllib_with_spaces() {
        assert_eq!(mtllib_of("mtllib my file.mtl\n"), Some("my file.mtl".into()));
    }
}
