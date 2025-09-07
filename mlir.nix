{ stdenv
, runCommand
, lib
, cmake
, coreutils
, python3
, git
, fetchFromGitHub
, ninja
, libatomic_ops
}:

stdenv.mkDerivation rec {
    pname = "llvm-project";
    version = "unstable-2023-05-02";
    requiredSystemFeatures = [ "big-parallel" ];
    nativeBuildInputs = [ cmake ninja python3 ];
    src = fetchFromGitHub {
      owner = "llvm";
      repo = pname;
      rev = "87f0227cb60147a26a1eeb4fb06e3b505e9c7261";
      hash = "sha256-ysyB/EYxi2qE9fD5x/F2zI4vjn8UDoo1Z9ukiIrjFGw";
    };
    cmakeDir = "../llvm";
    cmakeFlags = [
      "-DC_INCLUDE_DIRS=${stdenv.cc.libc.dev}/include"
      "-DLLVM_ENABLE_BINDINGS=OFF"
      "-DLLVM_ENABLE_OCAMLDOC=OFF"
      "-DLLVM_BUILD_EXAMPLES=OFF"
      "-DLLVM_ENABLE_PROJECTS=mlir;clang"
      "-DLLVM_TARGETS_TO_BUILD=host"
      "-DLLVM_INSTALL_UTILS=ON"
      "-DLLVM_TARGETS_TO_BUILD=host"
      "-DCMAKE_BUILD_TYPE=Release"
      "-DLLVM_BUILD_LLVM_DYLIB=ON"
      "-DLLVM_BUILD_TOOLS=ON"
      #"-DLLVM_BUILD_STATIC=ON"
    ];
    checkTarget = "check-mlir check-clang";
    }
