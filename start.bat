@echo off
echo ===================================================
echo   Aegis WAF Development Launcher (Windows)
echo ===================================================
echo.

echo Step 1: Checking and starting ClickHouse Database...
docker compose up -d clickhouse
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Failed to start ClickHouse Docker container. Please make sure Docker Desktop is running!
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo Step 2: Waiting 5 seconds for ClickHouse to initialize...
timeout /t 5 >nul

:: Set ClickHouse credentials for spawned processes to inherit
set CLICKHOUSE_USER=default
set CLICKHOUSE_PASSWORD=aegis

echo.
echo Step 3: Starting WAF Controller in a new window...
start "Aegis Controller" cmd /k cargo run -- controller

echo.
echo Step 4: Starting WAF Agent (connecting to Controller) in a new window...
start "Aegis Agent" cmd /k cargo run -- agent --controller http://localhost:8080

echo.
echo Step 5: Starting Dashboard Vite Dev Server in a new window...
start "Aegis Dashboard" cmd /c "cd dashboard && npm run dev"

echo.
echo All processes started! 
echo Dashboard UI available at: http://localhost:5173/
echo Controller API available at: http://localhost:8080/
echo ===================================================
pause
