use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use k8slab::vagrantfile;

/// Vagrantfile을 읽어 VirtualBox에 쿠버네티스 실습 환경을 만든다(베이그런트 없이).
#[derive(Parser)]
#[command(name = "k8slab", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Vagrantfile을 해석한 실행 계획을 JSON으로 보여 준다
    Plan {
        /// Vagrantfile 경로
        #[arg(short, long, default_value = "Vagrantfile")]
        file: PathBuf,
        /// `uname -m` 값(arm64, x86_64). 생략하면 이 컴퓨터의 값
        #[arg(long)]
        arch: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Plan { file, arch } => {
            let src = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{} 읽기 실패: {e}", file.display());
                    return ExitCode::FAILURE;
                }
            };
            let arch = arch.unwrap_or_else(|| vagrantfile::host_arch().to_string());
            match vagrantfile::plan(&src, &arch) {
                Ok(p) => {
                    println!("{}", serde_json::to_string_pretty(&p).unwrap());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{}: {e}", file.display());
                    ExitCode::FAILURE
                }
            }
        }
    }
}
