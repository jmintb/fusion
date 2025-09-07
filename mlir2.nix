
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
      rev = "87f0227cb60147a26a1eeb4fb06e3b505e9c7261";
      hash = "sha256-ysyB/EYxi2qE9fD5x/F2zI4vjn8UDoo1Z9ukiIrjFGw=";
    };
    cmakeDir = "../llvm";
    cmakeFlags = [
            "-GNinja"
            # Debug for debug builds
            "-DCMAKE_BUILD_TYPE=Release"
            # inst will be our installation prefix
            "-DCMAKE_INSTALL_PREFIX=../inst" # I know, this has to be patched still
            # change this to enable the projects you need
            "-DLLVM_ENABLE_PROJECTS=mlir"
            "-DLLVM_BUILD_EXAMPLES=ON"
            # this makes llvm only to produce code for the current platform, this saves CPU time, change it to what you need
            "-DLLVM_TARGETS_TO_BUILD=host"
            "-DLLVM_ENABLE_ASSERTIONS=ON"
            # libxml2 needs to be disabled because the LLVM build system ignores its .la
            # file and doesn't link zlib as well.
            # https://github.com/ClangBuiltLinux/tc-build/issues/150#issuecomment-845418812
            "-DLLVM_ENABLE_LIBXML2=OFF"
    ];
    checkTarget = "check-mlir check-clang";
    }
