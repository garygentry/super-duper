@echo off
dotnet build ui\windows\SuperDuper.sln -c Debug -p:Platform=x64
if %errorlevel% neq 0 exit /b %errorlevel%
ui\windows\SuperDuper\bin\x64\Debug\net10.0-windows10.0.22621.0\SuperDuper.exe
