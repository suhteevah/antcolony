@echo off
REM Build antcolony-trainer with the CUDA feature on kokonoe.
REM Requires: VS2022 cl.exe (via vcvars64), CUDA toolkit (nvcc), msvc Rust toolchain.
REM CUDA on Windows needs the MSVC target — gnu Rust cannot link nvcc/cudarc objects.
REM
REM NOTE: use the BuildTools vcvars64, NOT the Community one. The Community
REM install on kokonoe is missing vcvarsall.bat, so its vcvars64 stub fails
REM ("'...vcvarsall.bat' is not recognized"). BuildTools has the full set.

call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 (echo VCVARS FAILED & exit /b 1)

echo === cl ===
where cl
echo === link ===
where link
echo === nvcc ===
where nvcc
echo === rustc (msvc) ===
rustc +stable-x86_64-pc-windows-msvc --version

REM Linker: the global ~/.cargo/config.toml sets linker=lld-link, and LLVM
REM (lld-link) is now installed and on the machine PATH (2026-05-29), so no
REM per-build override is needed — the global config just works. vcvars
REM above still provides cl.exe (nvcc host compiler) + the MSVC LIB/INCLUDE
REM paths that lld-link links against.
echo === lld-link ===
where lld-link

echo === build trainer --features cuda ===
cargo +stable-x86_64-pc-windows-msvc build -p antcolony-trainer --features cuda
echo === EXITCODE %ERRORLEVEL% ===
