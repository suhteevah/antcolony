@echo off
REM Build + run the joint-PPO smoke on the GPU (CUDA) on kokonoe.
REM See build_trainer_cuda.bat for why we use BuildTools vcvars64 (Community
REM VS lacks vcvarsall.bat). lld-link (LLVM) is installed + on PATH, so the
REM global ~/.cargo config's linker=lld-link works without an override.
REM Runtime needs the CUDA bin (cudart/cublas/curand DLLs) on PATH —
REM CUDA_PATH\bin is already on the system PATH (nvcc resolves from it).

call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 (echo VCVARS FAILED & exit /b 1)

REM lld-link (LLVM) is installed + on PATH, so the global ~/.cargo config's
REM linker=lld-link works without a per-build override (2026-05-29).
set RUST_LOG=antcolony_trainer=info,joint_smoke=info

cargo +stable-x86_64-pc-windows-msvc run --bin joint_smoke -p antcolony-trainer --features cuda
echo === EXITCODE %ERRORLEVEL% ===
