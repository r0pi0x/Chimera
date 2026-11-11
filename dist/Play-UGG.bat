@echo off
rem Ultimate Gun Game: start the game and keep what its console says in ugg-last.log
rem (overwritten on each start). Options are passed on, e.g. Play-UGG.bat +connect host:27015
cd /d "%~dp0"
ultimate_gun_game.exe %* > ugg-last.log 2>&1
if errorlevel 1 echo The game stopped with an error: send ugg-last.log from this folder to the host.
if errorlevel 1 pause
