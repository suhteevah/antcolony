@echo off
REM Build + run Phase-3 training on the GPU (CUDA) on kokonoe. Forwards all
REM args to phase3_train (e.g. --iters 200 --envs 64 --eval-every 25
REM --matches-per-eval 50 --reward assets/reward/default.toml --out bench/phase3-a1).
REM
REM See build_trainer_cuda.bat for the env rationale (BuildTools vcvars64;
REM lld-link is installed + on PATH so no linker override needed). Runtime
REM needs the CUDA bin (cudart/cublas/curand DLLs) on PATH — CUDA_PATH\bin
REM is already on the system PATH.

call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 (echo VCVARS FAILED & exit /b 1)

set RUST_LOG=antcolony_trainer=info,phase3_train=info

cargo +stable-x86_64-pc-windows-msvc run --release --features cuda --bin phase3_train -- %*
echo === EXITCODE %ERRORLEVEL% ===
