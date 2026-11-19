//! Vagrantfile 해석기. 강의 저장소의 Vagrantfile이 쓰는 Ruby 문법만 다룬다.
//! 결과는 tools/oracle.rb(실제 Ruby로 실행한 결과)와 같아야 하고 tests/fixtures.rs가 이를 확인한다.

mod eval;
mod lexer;
mod parser;

use serde_json::Value;

/// Vagrantfile 내용을 읽어 실행 계획(JSON)을 만든다. `arch`는 `uname -m` 결과(arm64 또는 x86_64)다.
pub fn plan(src: &str, arch: &str) -> Result<Value, String> {
    let tokens = lexer::lex(src).map_err(|e| format!("토큰 나누기 실패: {e}"))?;
    let prog = parser::parse(tokens).map_err(|e| format!("구문 해석 실패: {e}"))?;
    eval::Evaluator::new(arch)
        .run(&prog)
        .map_err(|e| format!("실행 실패: {e}"))
}

/// 이 컴퓨터에서 `uname -m`이 돌려줄 값. Windows는 uname이 없지만 같은 기준으로 맞춘다.
pub fn host_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => other,
    }
}
