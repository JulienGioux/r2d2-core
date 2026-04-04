@echo off
setlocal

echo ==========================================
echo 🛡️  R2D2 Local CI Pipeline (Industrial-Grade)
echo ==========================================

echo [1/3] 💅 Checking Formatting (cargo fmt)...
cargo fmt --all -- --check
if %ERRORLEVEL% neq 0 (
    echo ❌ Formatting failed.
    exit /b %ERRORLEVEL%
)
echo ✅ Formatting OK.
echo.

echo [2/3] 🔍 Checking Lints (cargo clippy)...
echo Policy: Zero-Warning Policy (-D warnings)
cargo clippy --workspace --all-targets --all-features -- -D warnings
if %ERRORLEVEL% neq 0 (
    echo ❌ Lints failed. You have warnings or errors.
    exit /b %ERRORLEVEL%
)
echo ✅ Lints OK.
echo.

echo [3/3] 🧪 Running Test Suite (cargo test)...
cargo test --workspace --all-features
if %ERRORLEVEL% neq 0 (
    echo ❌ Tests failed.
    exit /b %ERRORLEVEL%
)
echo ✅ Tests OK.
echo.

echo ==========================================
echo 🚀 PASSED! Code is safe to push.
echo ==========================================
