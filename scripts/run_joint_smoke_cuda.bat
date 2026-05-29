@echo off
REM Build + run the joint-PPO smoke on the GPU (CUDA) on kokonoe.
REM See build_trainer_cuda.bat for why we use BuildTools vcvars64 + the
REM link.exe override (Community VS lacks vcvarsall.bat; the global
REM ~/.cargo/config.toml forces linker=lld-link which isn't installed).
REM Runtime needs the CUDA bin (cudart/cublas/curand DLLs) on PATH —
REM CUDA_PATH\bin is already on the system PATH (nvcc resolves from it).

call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 (echo VCVARS FAILED & exit /b 1)

set CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=link.exe
set RUST_LOG=antcolony_trainer=info,joint_smoke=info

cargo +stable-x86_64-pc-windows-msvc run --bin joint_smoke -p antcolony-trainer --features cuda
echo === EXITCODE %ERRORLEVEL% ===
