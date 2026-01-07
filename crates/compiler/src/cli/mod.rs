use anyhow::Result;
use clap::{command, Parser, Subcommand};

use crate::compiler;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: SubCommands,
}

#[derive(Subcommand)]
enum SubCommands {
    Run {
        path: Option<String>,
    },
    Build {
        #[arg(long)]
        emit_mlir: bool,
        #[arg(long)]
        emit_llvmir: bool,
        path: Option<String>,
        #[arg(long)]
        analyse: bool,
    },
}

fn load_std_lib() -> String {
    "

intrinsic fn write_bytes(owned value: ptr, owned destination: ptr, owned length: u64);
intrinsic fn pointer_from_offset(owned base_pointer: ptr, owned offset: i64) -> ptr;

extern fn free(let ptr: ptr);
extern fn malloc(owned size: usize) -> ptr;        
extern fn fdopen(owned fd: usize, owned mode: str) -> ptr;
extern fn fclose(owned fd: str) -> usize;
extern fn fwrite(owned val: ptr, owned size: i32, owned len: usize, owned file: str) -> usize;
extern fn sprintf(output: str, format: str, number: usize) -> usize;
extern fn fflush(owned file: str) -> usize;
extern fn sleep(time: usize) -> usize;
fn print(owned val: str, owned len: i32) {
     let std_out: usize= 1
     let character_size: usize = 1
     let stdoutptr = fdopen(std_out, \"w\");
     let res = fwrite(val, len, character_size, stdoutptr);
     let stdoutptrb = fdopen(std_out, \"w\");
     let resb = fflush(stdoutptrb);
     return;
    }
    "
    .to_string()
} // TODO: sort out proper booking of the stdout "file". Note we open stdout twice to appease the borrow checker until the language
  // can handle this case properly.

pub fn load_program(path: Option<String>) -> Result<String> {
    let path = path.unwrap_or("./main.ts".to_string());
    let std_lib = load_std_lib();
    Ok(format!("{std_lib}\n {}", std::fs::read_to_string(path)?))
}

pub fn load_program_without_std_lib(path: Option<String>) -> Result<String> {
    let path = path.unwrap_or("./main.ts".to_string());
    Ok(std::fs::read_to_string(path)?)
}

pub fn exec_cli() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        SubCommands::Run { path } => compiler::jit(&path.unwrap())?,
        SubCommands::Build {
            emit_mlir: _,
            emit_llvmir,
            path,
            analyse,
        } => {
            let engine = if analyse {
                compiler::compile_with_analysis(&path.unwrap())?
            } else {
                compiler::compile(&path.unwrap())?
            };

            if emit_llvmir {
                engine.dump_to_object_file("testllvm.ir");
            }
        }
    }

    Ok(())
}
