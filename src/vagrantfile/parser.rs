//! 토큰을 문장과 식으로 묶는다. Vagrantfile에 나오는 Ruby 문법만 다룬다.

use super::lexer::{StrPart, Tok, Token};
use std::fmt;

#[derive(Debug, Clone)]
pub enum StrSeg {
    Lit(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct Block {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Str(Vec<StrSeg>),
    Int(i64),
    Sym(String),
    Bool(bool),
    Nil,
    Var(String),
    Array(Vec<Expr>),
    Hash(Vec<(Expr, Expr)>),
    Range(Box<Expr>, Box<Expr>, bool),
    Backtick(String),
    Index(Box<Expr>, Box<Expr>),
    Call {
        recv: Option<Box<Expr>>,
        name: String,
        args: Vec<Expr>,
        kwargs: Vec<(String, Expr)>,
        block: Option<Block>,
        line: usize,
    },
    Eq(Box<Expr>, Box<Expr>, bool),
    Not(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Assign(String, Expr),
    AttrAssign {
        recv: Expr,
        attr: String,
        value: Expr,
        line: usize,
    },
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        otherwise: Option<Vec<Stmt>>,
    },
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}번째 줄: {}", self.line, self.msg)
    }
}

pub fn parse(tokens: Vec<Token>) -> Result<Vec<Stmt>, ParseError> {
    let mut p = Parser {
        t: tokens,
        i: 0,
        locals: vec![Vec::new()],
    };
    let body = p.stmts(&[])?;
    if !matches!(p.cur(), Tok::Eof) {
        return Err(p.err("여기서 끝나야 한다"));
    }
    Ok(body)
}

struct Parser {
    t: Vec<Token>,
    i: usize,
    /// 지역 변수 이름. `cv[0]`(변수 인덱스)과 `foo [0]`(메서드 인자)을 가르는 데 쓴다
    locals: Vec<Vec<String>>,
}

impl Parser {
    fn cur(&self) -> &Tok {
        &self.t[self.i].tok
    }
    fn tok(&self) -> &Token {
        &self.t[self.i]
    }
    fn line(&self) -> usize {
        self.t[self.i].line
    }
    fn bump(&mut self) -> Token {
        let t = self.t[self.i].clone();
        if !matches!(t.tok, Tok::Eof) {
            self.i += 1;
        }
        t
    }
    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError {
            line: self.line(),
            msg: format!("{} (만난 토큰: {:?})", msg.into(), self.cur()),
        }
    }
    fn is_punct(&self, p: &str) -> bool {
        matches!(self.cur(), Tok::Punct(x) if *x == p)
    }
    fn is_kw(&self, k: &str) -> bool {
        matches!(self.cur(), Tok::Kw(x) if *x == k)
    }
    fn expect_punct(&mut self, p: &str) -> Result<(), ParseError> {
        if self.is_punct(p) {
            self.bump();
            Ok(())
        } else {
            Err(self.err(format!("`{p}`가 와야 한다")))
        }
    }
    fn expect_kw(&mut self, k: &str) -> Result<(), ParseError> {
        if self.is_kw(k) {
            self.bump();
            Ok(())
        } else {
            Err(self.err(format!("`{k}`가 와야 한다")))
        }
    }
    fn skip_newlines(&mut self) {
        while matches!(self.cur(), Tok::Newline) {
            self.bump();
        }
    }
    fn is_local(&self, name: &str) -> bool {
        self.locals.iter().any(|s| s.iter().any(|n| n == name))
    }
    fn declare(&mut self, name: &str) {
        if !self.is_local(name) {
            self.locals.last_mut().unwrap().push(name.to_string());
        }
    }

    /// `stop`에 있는 키워드(end, elsif, else)나 파일 끝에서 멈춘다
    fn stmts(&mut self, stop: &[&str]) -> Result<Vec<Stmt>, ParseError> {
        let mut out = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.cur(), Tok::Eof) || stop.iter().any(|k| self.is_kw(k)) {
                return Ok(out);
            }
            out.push(self.stmt()?);
            if !(matches!(self.cur(), Tok::Newline | Tok::Eof)
                || stop.iter().any(|k| self.is_kw(k)))
            {
                return Err(self.err("문장이 여기서 끝나야 한다"));
            }
        }
    }

    fn stmt(&mut self) -> Result<Stmt, ParseError> {
        if self.is_kw("if") || self.is_kw("unless") {
            return self.if_stmt();
        }
        let line = self.line();
        let e = self.expr()?;
        if self.is_punct("=") {
            self.bump();
            self.skip_newlines();
            let value = self.expr()?;
            return match e {
                Expr::Var(name) => {
                    self.declare(&name);
                    Ok(Stmt::Assign(name, value))
                }
                Expr::Call {
                    recv: Some(recv),
                    name,
                    args,
                    kwargs,
                    block: None,
                    ..
                } if args.is_empty() && kwargs.is_empty() => Ok(Stmt::AttrAssign {
                    recv: *recv,
                    attr: name,
                    value,
                    line,
                }),
                Expr::Call {
                    recv: None,
                    name,
                    args,
                    kwargs,
                    block: None,
                    ..
                } if args.is_empty() && kwargs.is_empty() => {
                    self.declare(&name);
                    Ok(Stmt::Assign(name, value))
                }
                _ => Err(ParseError {
                    line,
                    msg: "대입할 수 없는 대상이다".into(),
                }),
            };
        }
        Ok(Stmt::Expr(e))
    }

    fn if_stmt(&mut self) -> Result<Stmt, ParseError> {
        let negate = self.is_kw("unless");
        self.bump();
        let mut cond = self.expr()?;
        if negate {
            cond = Expr::Not(Box::new(cond));
        }
        if self.is_kw("then") {
            self.bump();
        }
        let mut branches = vec![(cond, self.stmts(&["elsif", "else", "end"])?)];
        let mut otherwise = None;
        loop {
            if self.is_kw("elsif") {
                self.bump();
                let c = self.expr()?;
                if self.is_kw("then") {
                    self.bump();
                }
                branches.push((c, self.stmts(&["elsif", "else", "end"])?));
            } else if self.is_kw("else") {
                self.bump();
                otherwise = Some(self.stmts(&["end"])?);
            } else {
                self.expect_kw("end")?;
                return Ok(Stmt::If {
                    branches,
                    otherwise,
                });
            }
        }
    }

    fn expr(&mut self) -> Result<Expr, ParseError> {
        let l = self.add()?;
        if self.is_punct("==") || self.is_punct("!=") {
            let ne = self.is_punct("!=");
            self.bump();
            let r = self.add()?;
            return Ok(Expr::Eq(Box::new(l), Box::new(r), ne));
        }
        Ok(l)
    }

    fn add(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.unary()?;
        while self.is_punct("+") {
            self.bump();
            let r = self.unary()?;
            l = Expr::Add(Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.is_punct("!") {
            self.bump();
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.primary()?;
        loop {
            if self.is_punct(".") {
                self.bump();
                let line = self.line();
                let name = match self.bump().tok {
                    Tok::Ident(n) => n,
                    Tok::Label(n) => {
                        // `a.b:`는 문법에 없다
                        return Err(ParseError {
                            line,
                            msg: format!("메서드 이름 뒤에 `:`가 왔다: {n}"),
                        });
                    }
                    Tok::Kw(k) => k.to_string(),
                    other => {
                        return Err(ParseError {
                            line,
                            msg: format!("메서드 이름이 와야 한다: {other:?}"),
                        })
                    }
                };
                e = self.call_rest(Some(e), name, line)?;
            } else if self.is_punct("[") && !self.tok().space_before {
                self.bump();
                let idx = self.range_or_expr()?;
                self.expect_punct("]")?;
                e = Expr::Index(Box::new(e), Box::new(idx));
            } else {
                return Ok(e);
            }
        }
    }

    /// 메서드 이름 다음: 괄호 인자, 괄호 없는 인자, do 블록
    fn call_rest(
        &mut self,
        recv: Option<Expr>,
        name: String,
        line: usize,
    ) -> Result<Expr, ParseError> {
        let mut args = Vec::new();
        let mut kwargs = Vec::new();
        if self.is_punct("(") && !self.tok().space_before {
            self.bump();
            self.arg_list(&mut args, &mut kwargs, Some(")"))?;
            self.expect_punct(")")?;
        } else if self.starts_command_arg() {
            self.arg_list(&mut args, &mut kwargs, None)?;
        }
        let block = if self.is_kw("do") {
            Some(self.block()?)
        } else {
            None
        };
        Ok(Expr::Call {
            recv: recv.map(Box::new),
            name,
            args,
            kwargs,
            block,
            line,
        })
    }

    /// 괄호 없이 인자가 이어지는지(`cfg.vm.network "private_network", ip: ...`)
    fn starts_command_arg(&self) -> bool {
        if !self.tok().space_before {
            return false;
        }
        match self.cur() {
            Tok::Str(_)
            | Tok::Int(_)
            | Tok::Sym(_)
            | Tok::Label(_)
            | Tok::Ident(_)
            | Tok::Backtick(_) => true,
            Tok::Kw(k) => matches!(*k, "true" | "false" | "nil"),
            Tok::Punct(p) => matches!(*p, "[" | "(" | "{"),
            _ => false,
        }
    }

    fn arg_list(
        &mut self,
        args: &mut Vec<Expr>,
        kwargs: &mut Vec<(String, Expr)>,
        close: Option<&str>,
    ) -> Result<(), ParseError> {
        loop {
            if close.is_some_and(|c| self.is_punct(c)) {
                return Ok(());
            }
            if let Tok::Label(k) = self.cur().clone() {
                self.bump();
                kwargs.push((k, self.expr()?));
            } else if let Tok::Str(_) = self.cur() {
                // "k" => v 형태의 키워드 해시
                let save = self.i;
                let e = self.expr()?;
                if self.is_punct("=>") {
                    self.i = save;
                    let h = self.hash_pairs(close)?;
                    args.push(Expr::Hash(h));
                    return Ok(());
                }
                args.push(e);
            } else {
                args.push(self.expr()?);
            }
            if self.is_punct(",") {
                self.bump();
                if close.is_some() {
                    self.skip_newlines();
                }
            } else {
                return Ok(());
            }
        }
    }

    fn hash_pairs(&mut self, close: Option<&str>) -> Result<Vec<(Expr, Expr)>, ParseError> {
        let mut pairs = Vec::new();
        loop {
            self.skip_newlines();
            if self.is_punct("}") || close.is_some_and(|c| self.is_punct(c)) {
                return Ok(pairs);
            }
            if let Tok::Label(k) = self.cur().clone() {
                self.bump();
                pairs.push((Expr::Sym(k), self.expr()?));
            } else {
                let k = self.expr()?;
                self.expect_punct("=>")?;
                pairs.push((k, self.expr()?));
            }
            if self.is_punct(",") {
                self.bump();
            } else {
                return Ok(pairs);
            }
        }
    }

    fn block(&mut self) -> Result<Block, ParseError> {
        self.expect_kw("do")?;
        let mut params = Vec::new();
        if self.is_punct("|") {
            self.bump();
            loop {
                match self.bump().tok {
                    Tok::Ident(n) => params.push(n),
                    other => return Err(self.err(format!("블록 인자 이름이 와야 한다: {other:?}"))),
                }
                if self.is_punct(",") {
                    self.bump();
                } else {
                    break;
                }
            }
            self.expect_punct("|")?;
        }
        self.locals.push(params.clone());
        let body = self.stmts(&["end"]);
        self.locals.pop();
        let body = body?;
        self.expect_kw("end")?;
        Ok(Block { params, body })
    }

    fn range_or_expr(&mut self) -> Result<Expr, ParseError> {
        let a = self.expr()?;
        if self.is_punct("..") || self.is_punct("...") {
            let excl = self.is_punct("...");
            self.bump();
            let b = self.expr()?;
            return Ok(Expr::Range(Box::new(a), Box::new(b), excl));
        }
        Ok(a)
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let line = self.line();
        let t = self.bump();
        Ok(match t.tok {
            Tok::Int(n) => Expr::Int(n),
            Tok::Sym(s) => Expr::Sym(s),
            Tok::Backtick(s) => Expr::Backtick(s),
            Tok::Str(parts) => {
                let mut segs = Vec::new();
                for p in parts {
                    match p {
                        StrPart::Lit(s) => segs.push(StrSeg::Lit(s)),
                        StrPart::Interp(mut toks) => {
                            toks.push(Token {
                                tok: Tok::Eof,
                                line,
                                space_before: false,
                            });
                            let mut sub = Parser {
                                t: toks,
                                i: 0,
                                locals: self.locals.clone(),
                            };
                            let e = sub.expr()?;
                            if !matches!(sub.cur(), Tok::Eof) {
                                return Err(sub.err("#{ ... } 안에는 식 하나만 둔다"));
                            }
                            segs.push(StrSeg::Expr(e));
                        }
                    }
                }
                Expr::Str(segs)
            }
            Tok::Kw("true") => Expr::Bool(true),
            Tok::Kw("false") => Expr::Bool(false),
            Tok::Kw("nil") => Expr::Nil,
            Tok::Ident(name) => {
                if self.is_local(&name) && !(self.is_punct("(") && !self.tok().space_before) {
                    Expr::Var(name)
                } else if name.chars().next().is_some_and(|c| c.is_uppercase())
                    && !self.is_punct("(")
                {
                    // 상수(N, ARCH)와 모듈(Vagrant)
                    Expr::Var(name)
                } else {
                    self.call_rest(None, name, line)?
                }
            }
            Tok::Punct("(") => {
                self.skip_newlines();
                let e = self.range_or_expr()?;
                self.skip_newlines();
                self.expect_punct(")")?;
                e
            }
            Tok::Punct("[") => {
                let mut items = Vec::new();
                loop {
                    self.skip_newlines();
                    if self.is_punct("]") {
                        break;
                    }
                    items.push(self.expr()?);
                    self.skip_newlines();
                    if self.is_punct(",") {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.skip_newlines();
                self.expect_punct("]")?;
                Expr::Array(items)
            }
            Tok::Punct("{") => {
                let pairs = self.hash_pairs(None)?;
                self.skip_newlines();
                self.expect_punct("}")?;
                Expr::Hash(pairs)
            }
            other => {
                return Err(ParseError {
                    line,
                    msg: format!("식이 와야 한다: {other:?}"),
                })
            }
        })
    }
}
