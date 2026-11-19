//! Vagrantfile에 나오는 Ruby 문법만 토큰으로 나눈다.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Lit(String),
    Interp(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    /// `ip:`처럼 키워드 인자의 이름
    Label(String),
    Int(i64),
    Str(Vec<StrPart>),
    Sym(String),
    Backtick(String),
    Kw(&'static str),
    Punct(&'static str),
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    /// 바로 앞에 공백이 있었는지. `foo [1]`(인자)과 `foo[1]`(인덱스)을 가른다
    pub space_before: bool,
}

#[derive(Debug)]
pub struct LexError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}번째 줄: {}", self.line, self.msg)
    }
}

const KEYWORDS: &[&str] = &[
    "do", "end", "if", "elsif", "else", "unless", "then", "true", "false", "nil",
];
// 긴 것부터 맞춘다
const PUNCTS: &[&str] = &[
    "...", "..", "==", "!=", "=>", "(", ")", "[", "]", "{", "}", ",", ".", "=", "|", "!", "+",
];

pub fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    let chars: Vec<char> = src.chars().collect();
    let mut l = Lexer {
        c: chars,
        i: 0,
        line: 1,
    };
    let mut out = l.tokens(false)?;
    out.push(Token {
        tok: Tok::Eof,
        line: l.line,
        space_before: false,
    });
    Ok(out)
}

struct Lexer {
    c: Vec<char>,
    i: usize,
    line: usize,
}

impl Lexer {
    fn peek(&self, k: usize) -> Option<char> {
        self.c.get(self.i + k).copied()
    }

    fn err(&self, msg: impl Into<String>) -> LexError {
        LexError {
            line: self.line,
            msg: msg.into(),
        }
    }

    /// `in_interp`가 참이면 짝이 맞는 `}`에서 멈춘다(문자열 안의 `#{...}`).
    fn tokens(&mut self, in_interp: bool) -> Result<Vec<Token>, LexError> {
        let mut out: Vec<Token> = Vec::new();
        let mut depth: i32 = 0; // ( [ { 안에서는 줄바꿈을 무시한다
        let mut space = false;
        while let Some(ch) = self.peek(0) {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.i += 1;
                    space = true;
                    continue;
                }
                '\\' if self.peek(1) == Some('\n') => {
                    self.i += 2;
                    self.line += 1;
                    space = true;
                    continue;
                }
                '#' => {
                    while let Some(c) = self.peek(0) {
                        if c == '\n' {
                            break;
                        }
                        self.i += 1;
                    }
                    continue;
                }
                '\n' => {
                    self.i += 1;
                    let continues = depth > 0
                        || matches!(
                            out.last().map(|t| &t.tok),
                            Some(Tok::Punct(","))
                                | Some(Tok::Punct("=>"))
                                | Some(Tok::Punct("="))
                                | Some(Tok::Punct("."))
                                | Some(Tok::Punct("+"))
                        );
                    let line = self.line;
                    self.line += 1;
                    if !continues
                        && !matches!(out.last().map(|t| &t.tok), Some(Tok::Newline) | None)
                    {
                        out.push(Token {
                            tok: Tok::Newline,
                            line,
                            space_before: false,
                        });
                    }
                    space = true;
                    continue;
                }
                _ => {}
            }
            let line = self.line;
            let tok = if ch == '"' {
                self.i += 1;
                Tok::Str(self.dq_string()?)
            } else if ch == '\'' {
                self.i += 1;
                Tok::Str(vec![StrPart::Lit(self.sq_string()?)])
            } else if ch == '`' {
                self.i += 1;
                let mut s = String::new();
                loop {
                    match self.peek(0) {
                        None => return Err(self.err("` 문자열이 닫히지 않았다")),
                        Some('`') => {
                            self.i += 1;
                            break;
                        }
                        Some(c) => {
                            s.push(c);
                            self.i += 1;
                        }
                    }
                }
                Tok::Backtick(s)
            } else if ch.is_ascii_digit() {
                let mut s = String::new();
                while let Some(c) = self.peek(0) {
                    if c.is_ascii_digit() || c == '_' {
                        if c != '_' {
                            s.push(c);
                        }
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                Tok::Int(
                    s.parse()
                        .map_err(|_| self.err(format!("숫자를 읽지 못했다: {s}")))?,
                )
            } else if ch == ':' && self.peek(1).is_some_and(|c| c.is_alphabetic() || c == '_') {
                self.i += 1;
                Tok::Sym(self.ident())
            } else if ch.is_alphabetic() || ch == '_' {
                let id = self.ident();
                if self.peek(0) == Some(':') && self.peek(1) != Some(':') {
                    self.i += 1;
                    Tok::Label(id)
                } else if let Some(k) = KEYWORDS.iter().find(|k| **k == id) {
                    Tok::Kw(k)
                } else {
                    Tok::Ident(id)
                }
            } else if in_interp && ch == '}' && depth == 0 {
                self.i += 1;
                return Ok(out);
            } else if let Some(p) = PUNCTS.iter().find(|p| self.starts_with(p)) {
                self.i += p.chars().count();
                match *p {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    _ => {}
                }
                Tok::Punct(p)
            } else {
                return Err(self.err(format!("읽지 못하는 문자: {ch:?}")));
            };
            out.push(Token {
                tok,
                line,
                space_before: space,
            });
            space = false;
        }
        if in_interp {
            return Err(self.err("#{ ... }가 닫히지 않았다"));
        }
        Ok(out)
    }

    fn starts_with(&self, p: &str) -> bool {
        p.chars().enumerate().all(|(k, c)| self.peek(k) == Some(c))
    }

    fn ident(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek(0) {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        // strip!, empty? 같은 메서드 이름
        if let Some(c @ ('!' | '?')) = self.peek(0) {
            if self.peek(1) != Some('=') {
                s.push(c);
                self.i += 1;
            }
        }
        s
    }

    fn sq_string(&mut self) -> Result<String, LexError> {
        let mut s = String::new();
        loop {
            match self.peek(0) {
                None => return Err(self.err("' 문자열이 닫히지 않았다")),
                Some('\'') => {
                    self.i += 1;
                    return Ok(s);
                }
                Some('\\') if matches!(self.peek(1), Some('\'') | Some('\\')) => {
                    s.push(self.peek(1).unwrap());
                    self.i += 2;
                }
                Some(c) => {
                    if c == '\n' {
                        self.line += 1;
                    }
                    s.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn dq_string(&mut self) -> Result<Vec<StrPart>, LexError> {
        let mut parts = Vec::new();
        let mut s = String::new();
        loop {
            match self.peek(0) {
                None => return Err(self.err("\" 문자열이 닫히지 않았다")),
                Some('"') => {
                    self.i += 1;
                    break;
                }
                Some('\\') => {
                    let n = self.peek(1).ok_or_else(|| self.err("\\ 뒤가 비었다"))?;
                    self.i += 2;
                    match n {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '\n' => self.line += 1, // 줄 이어 쓰기: 둘 다 지운다
                        other => s.push(other),
                    }
                }
                Some('#') if self.peek(1) == Some('{') => {
                    self.i += 2;
                    if !s.is_empty() {
                        parts.push(StrPart::Lit(std::mem::take(&mut s)));
                    }
                    let inner = self.tokens(true)?;
                    parts.push(StrPart::Interp(inner));
                }
                Some(c) => {
                    if c == '\n' {
                        self.line += 1;
                    }
                    s.push(c);
                    self.i += 1;
                }
            }
        }
        if !s.is_empty() || parts.is_empty() {
            parts.push(StrPart::Lit(s));
        }
        Ok(parts)
    }
}
