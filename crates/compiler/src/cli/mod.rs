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

intrinsic fn write_bytes(owned value: ptr, owned destination: ptr, owned length: integer);
intrinsic fn pointer_from_offset(owned base_pointer: ptr, owned offset: integer) -> ptr;

extern fn free(let ptr: ptr);
extern fn malloc(owned size: integer) -> ptr;        
extern fn fdopen(owned fd: integer, owned mode: str) -> ptr;
extern fn fclose(owned fd: str) -> integer;
extern fn fwrite(owned val: ptr, owned size: integer, owned len: integer, owned file: str) -> integer;
extern fn sprintf(output: str, format: str, number: integer) -> integer;
extern fn fflush(owned file: str) -> integer;
extern fn sleep(time: integer) -> integer;
fn print(owned val: str, owned len: integer) {
     let stdoutptr = fdopen(1, \"w\");
     let res = fwrite(val, len, 1, stdoutptr);
     let stdoutptrb = fdopen(1, \"w\");
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
