//! 문장을 실행해 실행 계획(JSON)을 만든다. 결과 모양은 tools/oracle.rb와 같아야 한다.

use super::parser::{Block, Expr, Stmt, StrSeg};
use serde_json::{json, Map, Value as J};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub struct EvalError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "{}번째 줄: {}", self.line, self.msg)
        } else {
            write!(f, "{}", self.msg)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Obj {
    Vagrant,
    RootCfg,
    RootVm,
    Machine(usize),
    MachineVm(usize),
    Provider(usize, String),
    Ignored,
}

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Str(String),
    Int(i64),
    Sym(String),
    Bool(bool),
    Nil,
    Array(Vec<Val>),
    Hash(Vec<(Val, Val)>),
    Range(i64, i64, bool),
    Obj(Obj),
}

impl Val {
    /// Ruby의 to_s
    fn to_s(&self) -> String {
        match self {
            Val::Str(s) => s.clone(),
            Val::Int(n) => n.to_string(),
            Val::Sym(s) => s.clone(),
            Val::Bool(b) => b.to_string(),
            Val::Nil => String::new(),
            Val::Array(_) | Val::Hash(_) => self.inspect(),
            Val::Range(a, b, ex) => format!("{a}{}{b}", if *ex { "..." } else { ".." }),
            Val::Obj(o) => format!("{o:?}"),
        }
    }
    fn inspect(&self) -> String {
        match self {
            Val::Str(s) => format!("{s:?}"),
            Val::Sym(s) => format!(":{s}"),
            Val::Nil => "nil".into(),
            Val::Array(a) => format!(
                "[{}]",
                a.iter().map(|v| v.inspect()).collect::<Vec<_>>().join(", ")
            ),
            Val::Hash(h) => format!(
                "{{{}}}",
                h.iter()
                    .map(|(k, v)| format!("{}=>{}", k.inspect(), v.inspect()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            other => other.to_s(),
        }
    }
    fn truthy(&self) -> bool {
        !matches!(self, Val::Nil | Val::Bool(false))
    }
    /// oracle.rb의 plain(): 심볼은 ":id" 문자열로
    fn plain(&self) -> J {
        match self {
            Val::Str(s) => J::String(s.clone()),
            Val::Int(n) => json!(n),
            Val::Sym(s) => J::String(format!(":{s}")),
            Val::Bool(b) => J::Bool(*b),
            Val::Nil => J::Null,
            Val::Array(a) => J::Array(a.iter().map(|v| v.plain()).collect()),
            Val::Hash(h) => {
                let mut m = Map::new();
                for (k, v) in h {
                    let key = match k.plain() {
                        J::String(s) => s,
                        other => other.to_string(),
                    };
                    m.insert(key, v.plain());
                }
                J::Object(m)
            }
            other => J::String(other.to_s()),
        }
    }
}

pub struct Evaluator {
    /// `uname -m` 결과(arm64 또는 x86_64)
    arch: String,
    scopes: Vec<HashMap<String, Val>>,
    machines: Vec<Map<String, J>>,
    configured: bool,
    /// define 없이 config.vm에 바로 설정한 값이 들어가는 "default" VM(machines 안의 위치)
    default_idx: Option<usize>,
    defined: usize,
}

type R<T> = Result<T, EvalError>;

fn err<T>(line: usize, msg: impl Into<String>) -> R<T> {
    Err(EvalError {
        line,
        msg: msg.into(),
    })
}

impl Evaluator {
    pub fn new(arch: &str) -> Self {
        Evaluator {
            arch: arch.to_string(),
            scopes: vec![HashMap::new()],
            machines: Vec::new(),
            configured: false,
            default_idx: None,
            defined: 0,
        }
    }

    pub fn run(mut self, prog: &[Stmt]) -> R<J> {
        self.stmts(prog)?;
        if !self.configured {
            return err(0, "Vagrant.configure를 찾지 못했다");
        }
        // define 없이 config.vm에 바로 설정하면 베이그런트처럼 "default" VM 하나로 본다.
        // define과 섞어 쓰는 경우는 강의와 책에 없어 지원하지 않는다(tools/oracle.rb와 같은 규칙).
        if self.default_idx.is_some() && self.defined > 0 {
            return err(
                0,
                "define과 최상위 config.vm 설정을 함께 쓰는 Vagrantfile은 지원하지 않는다",
            );
        }
        let machines: Vec<J> = self.machines.into_iter().map(J::Object).collect();
        Ok(json!({ "machines": machines }))
    }

    fn get(&self, name: &str) -> Option<Val> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn set(&mut self, name: &str, v: Val) {
        for s in self.scopes.iter_mut().rev() {
            if let Some(slot) = s.get_mut(name) {
                *slot = v;
                return;
            }
        }
        self.scopes.last_mut().unwrap().insert(name.to_string(), v);
    }

    fn stmts(&mut self, body: &[Stmt]) -> R<()> {
        for s in body {
            self.stmt(s)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> R<()> {
        match s {
            Stmt::Expr(e) => {
                self.expr(e)?;
            }
            Stmt::Assign(name, e) => {
                let v = self.expr(e)?;
                self.set(name, v);
            }
            Stmt::AttrAssign {
                recv,
                attr,
                value,
                line,
            } => {
                let r = self.expr(recv)?;
                let v = self.expr(value)?;
                self.assign_attr(r, attr, v, *line)?;
            }
            Stmt::If {
                branches,
                otherwise,
            } => {
                for (cond, body) in branches {
                    if self.expr(cond)?.truthy() {
                        return self.stmts(body);
                    }
                }
                if let Some(body) = otherwise {
                    self.stmts(body)?;
                }
            }
        }
        Ok(())
    }

    fn run_block(&mut self, b: &Block, args: Vec<Val>) -> R<()> {
        let mut scope = HashMap::new();
        for (i, p) in b.params.iter().enumerate() {
            scope.insert(p.clone(), args.get(i).cloned().unwrap_or(Val::Nil));
        }
        self.scopes.push(scope);
        let r = self.stmts(&b.body);
        self.scopes.pop();
        r
    }

    fn expr(&mut self, e: &Expr) -> R<Val> {
        Ok(match e {
            Expr::Int(n) => Val::Int(*n),
            Expr::Sym(s) => Val::Sym(s.clone()),
            Expr::Bool(b) => Val::Bool(*b),
            Expr::Nil => Val::Nil,
            Expr::Str(segs) => {
                let mut s = String::new();
                for seg in segs {
                    match seg {
                        StrSeg::Lit(l) => s.push_str(l),
                        StrSeg::Expr(x) => s.push_str(&self.expr(x)?.to_s()),
                    }
                }
                Val::Str(s)
            }
            Expr::Var(name) => match self.get(name) {
                Some(v) => v,
                None if name == "Vagrant" => Val::Obj(Obj::Vagrant),
                None => return err(0, format!("정의되지 않은 이름: {name}")),
            },
            Expr::Array(items) => Val::Array(items.iter().map(|x| self.expr(x)).collect::<R<_>>()?),
            Expr::Hash(pairs) => {
                let mut h = Vec::new();
                for (k, v) in pairs {
                    h.push((self.expr(k)?, self.expr(v)?));
                }
                Val::Hash(h)
            }
            Expr::Range(a, b, ex) => match (self.expr(a)?, self.expr(b)?) {
                (Val::Int(a), Val::Int(b)) => Val::Range(a, b, *ex),
                (a, b) => return err(0, format!("정수 범위만 다룬다: {a:?}..{b:?}")),
            },
            Expr::Backtick(cmd) => {
                if cmd.trim() == "uname -m" {
                    Val::Str(format!("{}\n", self.arch))
                } else {
                    return err(
                        0,
                        format!("Vagrantfile이 지원하지 않는 명령을 실행한다: {cmd}"),
                    );
                }
            }
            Expr::Index(target, idx) => {
                let t = self.expr(target)?;
                let i = self.expr(idx)?;
                index(&t, &i)?
            }
            Expr::Eq(a, b, ne) => {
                let eq = self.expr(a)? == self.expr(b)?;
                Val::Bool(eq != *ne)
            }
            Expr::Not(a) => Val::Bool(!self.expr(a)?.truthy()),
            Expr::Add(a, b) => match (self.expr(a)?, self.expr(b)?) {
                (Val::Int(x), Val::Int(y)) => Val::Int(x + y),
                (Val::Str(x), Val::Str(y)) => Val::Str(x + &y),
                (Val::Array(mut x), Val::Array(y)) => {
                    x.extend(y);
                    Val::Array(x)
                }
                (x, y) => return err(0, format!("더할 수 없다: {x:?} + {y:?}")),
            },
            Expr::Call {
                recv,
                name,
                args,
                kwargs,
                block,
                line,
            } => {
                let r = match recv {
                    Some(r) => Some(self.expr(r)?),
                    None => None,
                };
                let a: Vec<Val> = args.iter().map(|x| self.expr(x)).collect::<R<_>>()?;
                let mut kw = Vec::new();
                for (k, v) in kwargs {
                    kw.push((k.clone(), self.expr(v)?));
                }
                self.call(r, name, a, kw, block.as_ref(), *line)?
            }
        })
    }

    fn call(
        &mut self,
        recv: Option<Val>,
        name: &str,
        args: Vec<Val>,
        kw: Vec<(String, Val)>,
        block: Option<&Block>,
        line: usize,
    ) -> R<Val> {
        let recv = match recv {
            Some(r) => r,
            None => match self.get(name) {
                Some(v) if args.is_empty() && kw.is_empty() && block.is_none() => return Ok(v),
                _ => return err(line, format!("지원하지 않는 메서드: {name}")),
            },
        };
        match recv {
            Val::Obj(o) => self.call_obj(o, name, args, kw, block, line),
            v => value_method(self, v, name, args, block, line),
        }
    }

    fn call_obj(
        &mut self,
        o: Obj,
        name: &str,
        args: Vec<Val>,
        kw: Vec<(String, Val)>,
        block: Option<&Block>,
        line: usize,
    ) -> R<Val> {
        match (&o, name) {
            (Obj::Vagrant, "configure") => {
                let b = match block {
                    Some(b) => b,
                    None => return err(line, "Vagrant.configure에 블록이 없다"),
                };
                self.configured = true;
                self.run_block(b, vec![Val::Obj(Obj::RootCfg)])?;
                Ok(Val::Nil)
            }
            (Obj::RootCfg, "vm") => Ok(Val::Obj(Obj::RootVm)),
            (Obj::RootCfg, _) => Ok(Val::Obj(Obj::Ignored)),
            (Obj::RootVm, "define") => {
                let vm_name = args.first().map(|v| v.to_s()).unwrap_or_default();
                let idx = self.new_machine(vm_name);
                self.defined += 1;
                if let Some(b) = block {
                    self.run_block(b, vec![Val::Obj(Obj::Machine(idx))])?;
                }
                Ok(Val::Nil)
            }
            (Obj::RootVm, _) => {
                let i = self.default_machine();
                self.call_obj(Obj::MachineVm(i), name, args, kw, block, line)
            }
            (Obj::Machine(i), "vm") => Ok(Val::Obj(Obj::MachineVm(*i))),
            (Obj::Machine(_), _) | (Obj::Ignored, _) => Ok(Val::Obj(Obj::Ignored)),
            (Obj::MachineVm(i), "network") => {
                let mut n = Map::new();
                n.insert(
                    "type".into(),
                    J::String(args.first().map(|v| v.to_s()).unwrap_or_default()),
                );
                for (k, v) in kw {
                    let val = if k == "guest" || k == "host" {
                        json!(to_int(&v, line)?)
                    } else {
                        v.plain()
                    };
                    n.insert(k, val);
                }
                self.push(*i, "networks", J::Object(n));
                Ok(Val::Nil)
            }
            (Obj::MachineVm(i), "synced_folder") => {
                let disabled = kw.iter().any(|(k, v)| k == "disabled" && v.truthy());
                let f = json!({
                    "src": args.first().map(|v| v.to_s()).unwrap_or_default(),
                    "dst": args.get(1).map(|v| v.to_s()).unwrap_or_default(),
                    "disabled": disabled,
                });
                self.push(*i, "synced_folders", f);
                Ok(Val::Nil)
            }
            (Obj::MachineVm(i), "provider") => {
                let pname = args.first().map(|v| v.to_s()).unwrap_or_default();
                let providers = self.machines[*i]
                    .get_mut("providers")
                    .unwrap()
                    .as_object_mut()
                    .unwrap();
                providers.insert(pname.clone(), json!({ "customize": [] }));
                if let Some(b) = block {
                    self.run_block(b, vec![Val::Obj(Obj::Provider(*i, pname))])?;
                }
                Ok(Val::Nil)
            }
            (Obj::MachineVm(i), "provision") => {
                let mut p = Map::new();
                p.insert(
                    "type".into(),
                    J::String(args.first().map(|v| v.to_s()).unwrap_or_default()),
                );
                for (k, v) in kw {
                    let val = match k.as_str() {
                        "args" => match &v {
                            Val::Array(a) => {
                                J::Array(a.iter().map(|x| J::String(x.to_s())).collect())
                            }
                            Val::Nil => json!([]),
                            other => json!([other.to_s()]),
                        },
                        "env" => match &v {
                            Val::Hash(h) => {
                                let mut m = Map::new();
                                for (ek, ev) in h {
                                    m.insert(ek.to_s(), J::String(ev.to_s()));
                                }
                                J::Object(m)
                            }
                            _ => return err(line, "env는 해시여야 한다"),
                        },
                        _ => v.plain(),
                    };
                    p.insert(k, val);
                }
                self.push(*i, "provisions", J::Object(p));
                Ok(Val::Nil)
            }
            (Obj::MachineVm(i), other) => {
                self.unsupported(*i, format!("vm.{other}"));
                Ok(Val::Nil)
            }
            (Obj::Provider(i, p), "customize") => {
                let arr = match args.first() {
                    Some(Val::Array(a)) => a
                        .iter()
                        .map(|x| match x.plain() {
                            J::String(s) => J::String(s),
                            other => J::String(other.to_string()),
                        })
                        .collect(),
                    _ => return err(line, "customize에는 배열을 준다"),
                };
                let prov = self.provider_mut(*i, p);
                prov.get_mut("customize")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .push(J::Array(arr));
                Ok(Val::Nil)
            }
            (Obj::Provider(i, p), other) => {
                let prov = self.provider_mut(*i, p);
                let list = prov.entry("unsupported").or_insert_with(|| json!([]));
                list.as_array_mut()
                    .unwrap()
                    .push(J::String(other.to_string()));
                Ok(Val::Nil)
            }
            (o, n) => err(line, format!("{o:?}에는 {n} 메서드가 없다")),
        }
    }

    fn assign_attr(&mut self, recv: Val, attr: &str, v: Val, line: usize) -> R<()> {
        let recv = match recv {
            Val::Obj(Obj::RootVm) => Val::Obj(Obj::MachineVm(self.default_machine())),
            other => other,
        };
        match recv {
            Val::Obj(Obj::MachineVm(i)) => {
                let (key, val) = match attr {
                    "box" => ("box", v.plain()),
                    "box_url" => ("box_url", v.plain()),
                    "box_version" => ("box_version", J::String(v.to_s())),
                    "host_name" | "hostname" => ("hostname", v.plain()),
                    other => {
                        self.unsupported(i, format!("vm.{other}="));
                        return Ok(());
                    }
                };
                self.machines[i].insert(key.into(), val);
            }
            Val::Obj(Obj::Provider(i, p)) => {
                let val = match attr {
                    "name" => J::String(v.to_s()),
                    "cpus" | "memory" => json!(to_int(&v, line)?),
                    "gui" => J::Bool(v.truthy()),
                    other => {
                        let prov = self.provider_mut(i, &p);
                        let list = prov.entry("unsupported").or_insert_with(|| json!([]));
                        list.as_array_mut()
                            .unwrap()
                            .push(J::String(format!("{other}=")));
                        return Ok(());
                    }
                };
                self.provider_mut(i, &p).insert(attr.into(), val);
            }
            Val::Obj(Obj::Ignored) | Val::Obj(Obj::RootCfg) | Val::Obj(Obj::Machine(_)) => {}
            other => return err(line, format!("{other:?}.{attr} = ...은 지원하지 않는다")),
        }
        Ok(())
    }

    fn new_machine(&mut self, name: String) -> usize {
        let mut m = Map::new();
        m.insert("name".into(), J::String(name));
        m.insert("networks".into(), json!([]));
        m.insert("synced_folders".into(), json!([]));
        m.insert("providers".into(), json!({}));
        m.insert("provisions".into(), json!([]));
        self.machines.push(m);
        self.machines.len() - 1
    }

    fn default_machine(&mut self) -> usize {
        match self.default_idx {
            Some(i) => i,
            None => {
                let i = self.new_machine("default".into());
                self.default_idx = Some(i);
                i
            }
        }
    }

    fn push(&mut self, i: usize, key: &str, v: J) {
        self.machines[i]
            .get_mut(key)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(v);
    }

    fn unsupported(&mut self, i: usize, what: String) {
        let list = self.machines[i]
            .entry("unsupported")
            .or_insert_with(|| json!([]));
        list.as_array_mut().unwrap().push(J::String(what));
    }

    fn provider_mut(&mut self, i: usize, p: &str) -> &mut Map<String, J> {
        self.machines[i]
            .get_mut("providers")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .get_mut(p)
            .unwrap()
            .as_object_mut()
            .unwrap()
    }
}

fn to_int(v: &Val, line: usize) -> R<i64> {
    match v {
        Val::Int(n) => Ok(*n),
        Val::Str(s) => s.trim().parse().map_err(|_| EvalError {
            line,
            msg: format!("정수가 아니다: {s:?}"),
        }),
        other => err(line, format!("정수가 아니다: {other:?}")),
    }
}

/// Ruby의 문자열, 배열 인덱스와 범위 잘라내기
fn index(t: &Val, i: &Val) -> R<Val> {
    fn bounds(len: usize, a: i64, b: i64, excl: bool) -> Option<(usize, usize)> {
        let len = len as i64;
        let a = if a < 0 { len + a } else { a };
        let mut b = if b < 0 { len + b } else { b };
        if !excl {
            b += 1;
        }
        if a < 0 || a > len {
            return None;
        }
        let b = b.clamp(a, len);
        Some((a as usize, b as usize))
    }
    match (t, i) {
        (Val::Str(s), Val::Int(n)) => {
            let c: Vec<char> = s.chars().collect();
            let k = if *n < 0 { c.len() as i64 + n } else { *n };
            Ok(c.get(k as usize)
                .filter(|_| k >= 0)
                .map(|ch| Val::Str(ch.to_string()))
                .unwrap_or(Val::Nil))
        }
        (Val::Str(s), Val::Range(a, b, ex)) => {
            let c: Vec<char> = s.chars().collect();
            Ok(match bounds(c.len(), *a, *b, *ex) {
                Some((x, y)) => Val::Str(c[x..y].iter().collect()),
                None => Val::Nil,
            })
        }
        (Val::Array(v), Val::Int(n)) => {
            let k = if *n < 0 { v.len() as i64 + n } else { *n };
            Ok(if k >= 0 {
                v.get(k as usize).cloned().unwrap_or(Val::Nil)
            } else {
                Val::Nil
            })
        }
        (Val::Array(v), Val::Range(a, b, ex)) => Ok(match bounds(v.len(), *a, *b, *ex) {
            Some((x, y)) => Val::Array(v[x..y].to_vec()),
            None => Val::Nil,
        }),
        (Val::Hash(h), k) => Ok(h
            .iter()
            .find(|(hk, _)| hk == k)
            .map(|(_, v)| v.clone())
            .unwrap_or(Val::Nil)),
        (t, i) => err(0, format!("인덱스를 지원하지 않는다: {t:?}[{i:?}]")),
    }
}

fn value_method(
    ev: &mut Evaluator,
    v: Val,
    name: &str,
    args: Vec<Val>,
    block: Option<&Block>,
    line: usize,
) -> R<Val> {
    match (&v, name) {
        (Val::Array(items), "each") | (Val::Array(items), "each_with_index") => {
            let b = block.ok_or_else(|| EvalError {
                line,
                msg: "each에 블록이 없다".into(),
            })?;
            for (k, it) in items.clone().into_iter().enumerate() {
                let mut a = vec![it];
                if name == "each_with_index" {
                    a.push(Val::Int(k as i64));
                }
                ev.run_block(b, a)?;
            }
            Ok(v)
        }
        (Val::Range(a, b, ex), "each") => {
            let blk = block.ok_or_else(|| EvalError {
                line,
                msg: "each에 블록이 없다".into(),
            })?;
            let end = if *ex { *b - 1 } else { *b };
            let mut k = *a;
            while k <= end {
                ev.run_block(blk, vec![Val::Int(k)])?;
                k += 1;
            }
            Ok(v)
        }
        (Val::Int(n), "times") => {
            let blk = block.ok_or_else(|| EvalError {
                line,
                msg: "times에 블록이 없다".into(),
            })?;
            for k in 0..*n {
                ev.run_block(blk, vec![Val::Int(k)])?;
            }
            Ok(v)
        }
        (Val::Range(a, b, ex), "to_a") => {
            let end = if *ex { *b - 1 } else { *b };
            Ok(Val::Array((*a..=end).map(Val::Int).collect()))
        }
        (Val::Str(s), "strip") => Ok(Val::Str(s.trim().to_string())),
        // Ruby의 strip!은 바뀐 것이 없으면 nil을 돌려준다
        (Val::Str(s), "strip!") => Ok(if s.trim() == s {
            Val::Nil
        } else {
            Val::Str(s.trim().to_string())
        }),
        (Val::Str(s), "chomp") => Ok(Val::Str(s.strip_suffix('\n').unwrap_or(s).to_string())),
        (Val::Str(s), "upcase") => Ok(Val::Str(s.to_uppercase())),
        (Val::Str(s), "downcase") => Ok(Val::Str(s.to_lowercase())),
        (_, "to_s") => Ok(Val::Str(v.to_s())),
        (Val::Str(s), "to_i") => Ok(Val::Int(s.trim().parse().unwrap_or(0))),
        (Val::Int(_), "to_i") => Ok(v),
        (Val::Str(s), "length") | (Val::Str(s), "size") => Ok(Val::Int(s.chars().count() as i64)),
        (Val::Array(a), "length") | (Val::Array(a), "size") => Ok(Val::Int(a.len() as i64)),
        (Val::Array(a), "join") => {
            let sep = args.first().map(|x| x.to_s()).unwrap_or_default();
            Ok(Val::Str(
                a.iter().map(|x| x.to_s()).collect::<Vec<_>>().join(&sep),
            ))
        }
        _ => err(
            line,
            format!("지원하지 않는 메서드: {}.{name}", v.inspect()),
        ),
    }
}
