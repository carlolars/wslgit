@echo off
net session >nul 2>&1
if %ERRORLEVEL% neq 0 (
   echo.
   echo This script must be run as administrator to work properly!
   echo Right click on the script and select "Run as administrator".
   echo.
   goto :error
)

set CWD=%~dp0
cd %CWD%

echo.
echo Make sure Fork.RI is executable in WSL...
wsl -- chmod +x cmd/Fork.RI
if %ERRORLEVEL% neq 0 (
    echo ERROR! Failed to make Fork.RI executable in WSL.
    goto :error
)

echo.
if exist "%CWD%cmd\git.exe" (
    echo 'git.exe' already exist.
) else (
    echo Create 'git.exe' symlink...
    mklink %CWD%cmd\git.exe %CWD%cmd\wslgit.exe
    if %ERRORLEVEL% neq 0 (
        echo ERROR! Failed to create symlink.
        goto :error
    )
)

echo.
if exist "%CWD%cmd\sh.exe" (
    echo 'sh.exe' already exist.
) else (
    echo Create 'sh.exe' symlink...
    mklink %CWD%cmd\sh.exe C:\Windows\System32\bash.exe
    if %ERRORLEVEL% neq 0 (
        echo ERROR! Failed to create symlink.
        goto :error
    )
)

echo.
if exist "%CWD%cmd\bash.exe" (
    echo 'bash.exe' already exist.
) else (
    echo Create 'bash.exe' symlink...
    mklink %CWD%cmd\bash.exe C:\Windows\System32\bash.exe
    if %ERRORLEVEL% neq 0 (
        echo ERROR! Failed to create symlink.
        goto :error
    )
)

echo.
echo Installation successful!
echo.
echo Add to the Windows Path environment variable (user or system) to use as system git:
echo  %CWD%cmd
echo.
pause
exit /B 0

:error
pause
exit /B 1
