@echo off
rem Skill preview viewer: double-click to open it in the browser.
rem Keep this window open while you use the viewer; close it when you are done.
python "%~dp0preview_viewer.py" %*
if errorlevel 1 pause
