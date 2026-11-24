@echo off
title Riva-Translate server (port 47870)
"%~dp0runtime\llama\llama-server.exe" -m "%~dp0models\riva\Riva-Translate-4B-Instruct-v2-Q4_K_M.gguf" --host 127.0.0.1 --port 47870 -c 2048 -t 8
pause
