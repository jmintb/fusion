{ stdenv
, runCommand
, lib
, cmake
, coreutils
, python3
, git
, fetchFromGitHub
, ninja
}:

stdenv.mkDerivation rec {
    pname = "llvm-project";
    version = "unstable-2023-05-02";
    requiredSystemFeatures = [ "big-parallel" ];
    nativeBuildInputs = [ cmake ninja python3 ];
    src = fetchFromGitHub {
      owner = "llvm";
      repo = pname;
      rev = "3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff";
      hash = "sha256-iiZKMRo/WxJaBXct9GdAcAT3cz9d9pnAcO1mmR6oPNE=";
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
    ];
    checkTarget = "check-mlir check-clang";
    }
