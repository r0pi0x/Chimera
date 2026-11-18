//! The text scanners' shared source reader: `strip` returns a file's code
//! without comments, string and char literals (strings left as `""`) and
//! without its `#[cfg(test)]` items. A column-0 `#[cfg(test)]` drops only
//! the item it marks (a `mod name { .. }` is brace-matched, a `use` or
//! `mod name;` ends at its `;`), so production code below an inline test
//! module is still scanned. Newlines are kept, so line numbers hold.
//! Every scanner in `checks/tests/it/` goes through it; none keeps its own
//! cut.

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Blanks every item marked by a column-0 `#[cfg(test)]` in already
/// stripped code (no comments or strings, so braces count right).
fn drop_test_items(code: &str) -> String {
    const ATTR: &str = "#[cfg(test)]";
    let mut out = String::with_capacity(code.len());
    let mut rest = 0;
    let mut from = 0;
    while let Some(p) = code[from..].find(ATTR) {
        let at = from + p;
        if at != 0 && !code[..at].ends_with('\n') {
            from = at + ATTR.len();
            continue;
        }
        let end = item_end(code, at + ATTR.len());
        out.push_str(&code[rest..at]);
        out.extend(code[at..end].chars().filter(|&c| c == '\n'));
        rest = end;
        from = end;
    }
    out.push_str(&code[rest..]);
    out
}

/// Where the item that starts at `from` ends: after its first top-level
/// `{ .. }` group or its `;`, whichever comes first.
fn item_end(code: &str, from: usize) -> usize {
    let mut paren = 0i32;
    for (i, c) in code[from..].char_indices() {
        match c {
            '(' | '[' => paren += 1,
            ')' | ']' => paren -= 1,
            ';' if paren <= 0 => return from + i + 1,
            '{' if paren <= 0 => {
                let mut depth = 0i32;
                for (j, d) in code[from + i..].char_indices() {
                    match d {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                return from + i + j + 1;
                            }
                        }
                        _ => {}
                    }
                }
                return code.len();
            }
            _ => {}
        }
    }
    code.len()
}

/// Source without comments, string and char literals (strings left as
/// `""`) and without its column-0 `#[cfg(test)]` items (see
/// `drop_test_items`); the code after an inline test module stays.
pub(crate) fn strip(src: &str) -> String {
    drop_test_items(&strip_literals(src))
}

fn strip_literals(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if b[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
        } else if (c == 'r' || c == 'b')
            && !out.chars().next_back().is_some_and(is_ident)
            && raw_string_start(&b, i).is_some()
        {
            let (hashes, start) = raw_string_start(&b, i).unwrap();
            let close: Vec<char> = std::iter::once('"').chain(std::iter::repeat_n('#', hashes)).collect();
            i = start;
            while i < b.len() && !b[i..].starts_with(&close) {
                i += 1;
            }
            i += close.len();
            out.push_str("\"\"");
        } else if c == '"' {
            i += 1;
            while i < b.len() && b[i] != '"' {
                i += if b[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("\"\"");
        } else if c == '\'' && (next == Some('\\') || b.get(i + 2) == Some(&'\'')) {
            i += 1;
            while i < b.len() && b[i] != '\'' {
                i += if b[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("''");
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// At a raw string (`r"`, `r#"`, `br"`): its `#` count and where its text
/// starts.
fn raw_string_start(b: &[char], mut i: usize) -> Option<(usize, usize)> {
    if b[i] == 'b' {
        i += 1;
    }
    if b.get(i) != Some(&'r') {
        return None;
    }
    i += 1;
    let mut hashes = 0;
    while b.get(i) == Some(&'#') {
        hashes += 1;
        i += 1;
    }
    (b.get(i) == Some(&'"')).then_some((hashes, i + 1))
}


#[test]
fn code_below_an_inline_test_module_is_still_scanned() {
    let src = "fn a() {}\n\
               #[cfg(test)]\n\
               mod tests {\n    fn t() { let s = \"}\"; if x { y } } // }\n}\n\
               fn below() { HashSet::new() }\n\
               #[cfg(test)]\nuse foo::Bar;\n\
               fn last() {}\n\
               #[cfg(test)]\nmod more;\n";
    let code = strip(src);
    assert!(code.contains("fn a()") && code.contains("fn below()") && code.contains("fn last()"));
    assert!(!code.contains("fn t()") && !code.contains("Bar") && !code.contains("cfg(test)"));
    assert_eq!(code.lines().count(), src.lines().count());
}
