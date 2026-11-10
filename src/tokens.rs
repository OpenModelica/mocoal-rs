// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the GNU AGPL version 3 license or the OSMC Public
// License (OSMC-PL) version 1.8, according to the recipient's choice. See LICENSE.md.

//! The token stream of a source file, and text edits addressed by tokens.
//!
//! The Absyn tree gives what a class contains. The tokens give where: every change to the source
//! is an [`Edit`] of a byte range found through the tokens, so comments and formatting stay as
//! they are.
//!
//! Mirrors `src/tokens.jl` of ModelicaComponentAutolayout.jl.

use openmodelica_ast::parser::Grammar;
use openmodelica_ast::parser::lexer;
pub use openmodelica_ast::parser::lexer::TokenKind;

/// A source file: its text and its tokens, each with its byte range in the text.
pub struct Source {
    pub text: String,
    pub file_name: String,
    kinds: Vec<TokenKind>,
    /// Line and character column of the first character, both 1-based, as the parser records them.
    positions: Vec<(u32, u32)>,
    starts: Vec<usize>,
    stops: Vec<usize>,
}

/// Replace the bytes `start..stop` of the source by `text`. `start == stop` is an insertion.
pub struct Edit {
    pub start: usize,
    pub stop: usize,
    pub text: String,
}

/// One declaration of a component clause (`T a, b;` has two): the tokens `first` to `delim - 1`.
/// `delim` is the `,` or `;` that ends it. `ann_tok` is the `annotation` token of its comment and
/// `cc_ann_tok` the one of a constraining clause.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub first: usize,
    pub delim: usize,
    pub ann_tok: Option<usize>,
    pub cc_tok: Option<usize>,
    pub cc_ann_tok: Option<usize>,
}

/// An argument of a modification list: its name and its tokens `first..=last`.
#[derive(Clone, Debug)]
pub struct Arg {
    pub name: String,
    pub first: usize,
    pub last: usize,
}

/// The number of bytes of the token that starts at byte `at`.
fn token_length(text: &str, at: usize, kind: &TokenKind) -> usize {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let digits = |mut i: usize| {
        while i < n && bytes[i].is_ascii_digit() {
            i += 1;
        }
        i
    };
    let exponent = |i: usize| {
        if i < n && (bytes[i] == b'e' || bytes[i] == b'E') {
            let mut j = i + 1;
            if j < n && (bytes[j] == b'+' || bytes[j] == b'-') {
                j += 1;
            }
            if j < n && bytes[j].is_ascii_digit() {
                return digits(j);
            }
        }
        i
    };
    let first = bytes[at];
    let stop = match first {
        b'"' | b'\'' => {
            let mut i = at + 1;
            loop {
                if i >= n {
                    break n;
                } else if bytes[i] == b'\\' {
                    i += 2;
                } else if bytes[i] == first {
                    break i + 1;
                } else {
                    i += 1;
                }
            }
        }
        b'0'..=b'9' => {
            let mut i = digits(at);
            if i < n && bytes[i] == b'.' && !(i + 1 < n && bytes[i + 1] == b'.') {
                i = digits(i + 1);
            }
            exponent(i)
        }
        b'.' if at + 1 < n && bytes[at + 1].is_ascii_digit() => exponent(digits(at + 1)),
        b'&' | b'|' if at + 1 < n && bytes[at + 1] == first => at + 2,
        b'!' => at + 1,
        _ if first == b'$' || first == b'_' || first.is_ascii_alphabetic() => {
            let mut i = at + 1;
            for (k, c) in text[at + 1..].char_indices() {
                if !(c.is_alphanumeric() || c == '_') {
                    break;
                }
                i = at + 1 + k + c.len_utf8();
            }
            i
        }
        _ => match kind {
            TokenKind::BOM => at + '\u{FEFF}'.len_utf8(),
            _ => at + kind.source_char_len() as usize,
        },
    };
    stop.min(n) - at
}

impl Source {
    /// Tokenize `text`. The error is the lexer's message.
    pub fn new(text: &str, file_name: &str) -> Result<Source, String> {
        let tokens = lexer::lex(text, Grammar::Modelica3).map_err(|e| e.to_string())?;
        let mut src = Source {
            text: text.to_owned(),
            file_name: file_name.to_owned(),
            kinds: Vec::with_capacity(tokens.len()),
            positions: Vec::with_capacity(tokens.len()),
            starts: Vec::with_capacity(tokens.len()),
            stops: Vec::with_capacity(tokens.len()),
        };
        // The lexer gives line and character column; the edits need bytes.
        let mut line: u32 = 1;
        let mut col: u32 = 1;
        let mut byte: usize = 0;
        let mut chars = text.chars();
        for t in tokens {
            while (line, col) < (t.line, t.col) {
                let Some(c) = chars.next() else {
                    return Err(format!("{file_name}: token at {}:{} is beyond the text", t.line, t.col));
                };
                byte += c.len_utf8();
                if c == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
            if (line, col) != (t.line, t.col) {
                return Err(format!("{file_name}: no character at {}:{}", t.line, t.col));
            }
            src.starts.push(byte);
            src.stops.push(byte + token_length(text, byte, &t.kind));
            src.positions.push((t.line, t.col));
            src.kinds.push(t.kind);
        }
        Ok(src)
    }

    pub fn token_count(&self) -> usize {
        self.kinds.len()
    }

    pub fn kind(&self, i: usize) -> &TokenKind {
        &self.kinds[i]
    }

    /// The first byte of token `i`.
    pub fn start(&self, i: usize) -> usize {
        self.starts[i]
    }

    /// One past the last byte of token `i`.
    pub fn stop(&self, i: usize) -> usize {
        self.stops[i]
    }

    /// The text of token `i`: the identifier, the literal, the string contents.
    pub fn token_text(&self, i: usize) -> &str {
        let (a, b) = (self.starts[i], self.stops[i]);
        if matches!(self.kinds[i], TokenKind::Str(_)) && b >= a + 2 {
            &self.text[a + 1..b - 1]
        } else {
            &self.text[a..b]
        }
    }

    /// The source text from the first byte of token `i` to the last byte of token `j`.
    pub fn token_slice(&self, i: usize, j: usize) -> &str {
        let (a, b) = (self.starts[i], self.stops[j]);
        if a > b { "" } else { &self.text[a..b] }
    }

    /// The token that starts at `line` and the 1-based character `column` (the start position of
    /// a `SourceInfo`).
    pub fn token_at(&self, line: i32, column: i32) -> Option<usize> {
        if line < 1 || column < 1 {
            return None;
        }
        self.positions.binary_search(&(line as u32, column as u32)).ok()
    }

    /// The last token that starts at or before `line` and the character `column`.
    pub fn token_before(&self, line: i32, column: i32) -> Option<usize> {
        if line < 1 {
            return None;
        }
        let at = (line as u32, column.max(0) as u32);
        self.positions.partition_point(|p| *p <= at).checked_sub(1)
    }

    /// The token at the end position of a `SourceInfo`: the token after the construct. The parser
    /// records a column that is one less on the first line (as ANTLR did).
    pub fn token_at_end(&self, line: i32, column: i32) -> Option<usize> {
        self.token_at(line, if line == 1 { column + 1 } else { column })
    }

    /// The token that closes the bracket opened by token `i`.
    pub fn match_close(&self, i: usize) -> Option<usize> {
        let mut depth: i32 = 0;
        for j in i..self.kinds.len() {
            if is_open(&self.kinds[j]) {
                depth += 1;
            } else if is_close(&self.kinds[j]) {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
        }
        None
    }

    /// The declarations of the element that starts at token `s`.
    pub fn element_segments(&self, s: usize) -> Vec<Segment> {
        let mut segs: Vec<Segment> = Vec::new();
        let mut depth: i32 = 0;
        let mut seg = Segment {
            first: s,
            delim: s,
            ann_tok: None,
            cc_tok: None,
            cc_ann_tok: None,
        };
        for j in s..self.kinds.len() {
            let k = &self.kinds[j];
            if is_open(k) {
                depth += 1;
            } else if is_close(k) {
                depth -= 1;
            } else if depth == 0 {
                match k {
                    TokenKind::Comma => {
                        segs.push(Segment { delim: j, ..seg });
                        seg = Segment {
                            first: j + 1,
                            delim: j + 1,
                            ann_tok: None,
                            cc_tok: None,
                            cc_ann_tok: None,
                        };
                    }
                    TokenKind::Semi => {
                        segs.push(Segment { delim: j, ..seg });
                        break;
                    }
                    TokenKind::Constrainedby => seg.cc_tok = Some(j),
                    TokenKind::Annotation if seg.cc_tok.is_none() => seg.ann_tok = Some(j),
                    TokenKind::Annotation => seg.cc_ann_tok = Some(j),
                    _ => {}
                }
            }
        }
        segs
    }

    /// The `;` that ends the equation starting at token `s`, and its `annotation` token.
    pub fn equation_end(&self, s: usize) -> (Option<usize>, Option<usize>) {
        let mut depth: i32 = 0;
        let mut ann: Option<usize> = None;
        for j in s..self.kinds.len() {
            let k = &self.kinds[j];
            if is_open(k) {
                depth += 1;
            } else if is_close(k) {
                depth -= 1;
            } else if depth == 0 {
                match k {
                    TokenKind::Semi => return (Some(j), ann),
                    TokenKind::Annotation => ann = Some(j),
                    _ => {}
                }
            }
        }
        (None, ann)
    }

    /// The token with the name of the argument `first..=last`.
    pub fn arg_name_token(&self, first: usize, last: usize) -> Option<usize> {
        let mut i = first;
        while i <= last && is_arg_prefix(&self.kinds[i]) {
            i += 1;
        }
        (i <= last && matches!(self.kinds[i], TokenKind::Ident(_))).then_some(i)
    }

    /// The arguments between the `(` at token `lpar` and its `)`, and the token of that `)`.
    pub fn args_of(&self, lpar: usize) -> Option<(Vec<Arg>, usize)> {
        let rpar = self.match_close(lpar)?;
        let mut args: Vec<Arg> = Vec::new();
        let mut first = lpar + 1;
        let mut depth: i32 = 0;
        for j in lpar + 1..=rpar {
            let k = &self.kinds[j];
            if j == rpar || (depth == 0 && matches!(k, TokenKind::Comma)) {
                if first < j {
                    let name = self.arg_name_token(first, j - 1).map(|t| self.token_text(t).to_owned());
                    args.push(Arg {
                        name: name.unwrap_or_default(),
                        first,
                        last: j - 1,
                    });
                }
                first = j + 1;
            } else if is_open(k) {
                depth += 1;
            } else if is_close(k) {
                depth -= 1;
            }
        }
        Some((args, rpar))
    }

    /// Set the modifier `path` in the argument list that opens at token `lpar` to the text `leaf`
    /// (the whole argument, its name included). The argument is replaced when it exists, and added
    /// to the innermost list that exists otherwise. Returns false when the list is not closed.
    pub fn set_modifier(&self, edits: &mut Vec<Edit>, lpar: usize, path: &[&str], level: usize, leaf: &str) -> bool {
        let Some((args, rpar)) = self.args_of(lpar) else {
            return false;
        };
        for a in &args {
            if a.name != path[level] {
                continue;
            }
            if level + 1 == path.len() {
                edits.push(Edit {
                    start: self.start(a.first),
                    stop: self.stop(a.last),
                    text: leaf.to_owned(),
                });
                return true;
            }
            if let Some(name_tok) = self.arg_name_token(a.first, a.last)
                && name_tok < a.last
                && matches!(self.kinds[name_tok + 1], TokenKind::LParen)
            {
                return self.set_modifier(edits, name_tok + 1, path, level + 1, leaf);
            }
            let text = nested_text(path, level, leaf);
            edits.push(Edit {
                start: self.start(a.first),
                stop: self.stop(a.last),
                text,
            });
            return true;
        }
        let text = nested_text(path, level, leaf);
        match args.last() {
            None => edits.push(Edit {
                start: self.start(rpar),
                stop: self.start(rpar),
                text,
            }),
            Some(a) => edits.push(Edit {
                start: self.stop(a.last),
                stop: self.stop(a.last),
                text: format!(", {text}"),
            }),
        }
        true
    }

    /// Remove the argument `name` from the list that opens at token `lpar`, with one of the commas
    /// next to it. Returns the number of arguments left, or `None` when nothing was removed.
    pub fn remove_argument(&self, edits: &mut Vec<Edit>, lpar: usize, name: &str) -> Option<usize> {
        let (args, _) = self.args_of(lpar)?;
        let i = args.iter().position(|a| a.name == name)?;
        let (start, stop) = if i + 1 < args.len() {
            (self.start(args[i].first), self.start(args[i + 1].first))
        } else if i > 0 {
            (self.stop(args[i - 1].last), self.stop(args[i].last))
        } else {
            (self.start(args[i].first), self.stop(args[i].last))
        };
        edits.push(Edit {
            start,
            stop,
            text: String::new(),
        });
        Some(args.len() - 1)
    }
}

pub fn is_open(k: &TokenKind) -> bool {
    matches!(k, TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace)
}

pub fn is_close(k: &TokenKind) -> bool {
    matches!(k, TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace)
}

fn is_arg_prefix(k: &TokenKind) -> bool {
    matches!(
        k,
        TokenKind::Each | TokenKind::Final | TokenKind::Redeclare | TokenKind::Replaceable
    )
}

/// `path[from]( ... path[last - 1](leaf) ... )`: the text of a nested modifier.
pub fn nested_text(path: &[&str], from: usize, leaf: &str) -> String {
    let mut s = leaf.to_owned();
    for name in path[from..path.len() - 1].iter().rev() {
        s = format!("{name}({s})");
    }
    s
}

/// Apply the edits to the text. Edits are applied in the order of their start; insertions at the
/// same byte keep the order in which they were added. An edit that overlaps an earlier one is
/// skipped and counted in the second value.
pub fn apply_edits(text: &str, edits: &[Edit]) -> (String, usize) {
    let mut order: Vec<usize> = (0..edits.len()).collect();
    order.sort_by_key(|&k| edits[k].start);
    let mut out = String::with_capacity(text.len() + edits.iter().map(|e| e.text.len()).sum::<usize>());
    let mut cursor: usize = 0;
    let mut skipped: usize = 0;
    for k in order {
        let e = &edits[k];
        if e.start < cursor || e.stop > text.len() || e.stop < e.start {
            skipped += 1;
            continue;
        }
        out.push_str(&text[cursor..e.start]);
        out.push_str(&e.text);
        cursor = e.stop;
    }
    out.push_str(&text[cursor..]);
    (out, skipped)
}
