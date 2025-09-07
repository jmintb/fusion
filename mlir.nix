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
      rev = "f780955e1df9105e9c4e67ebd16efded7dd279e2";
      hash = "sha256-gmRVOTLeCbxYIyLcOIQhYWOvx2Gbxq1DX60XSkGXI/8=";
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
