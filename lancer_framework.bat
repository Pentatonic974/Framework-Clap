@echo off
REM ======================================================================
REM  lancer_framework.bat - demarrage de l'app CLAP Plugin Studio
REM
REM  - build debug si l'exe n'existe pas encore (premier lancement)
REM  - relance l'exe sans recompiler ensuite (rapide, pour vos tests UI)
REM  - option  "rebuild"  (lancer_framework.bat rebuild) : force la
REM    recompilation avant lancement (apres une modif du code Dart)
REM ======================================================================
setlocal
title CLAP Plugin Studio - lanceur

REM --- racine du projet = repertoire de ce .bat (independant du cwd) ---
pushd "%~dp0"
set ROOT=%CD%
set EXE=%ROOT%\flutter_app\build\windows\x64\runner\Debug\claps_studio.exe

REM --- FORCE_BUILD=1 si l'utilisateur a passe "rebuild" en argument ---
set FORCE_BUILD=0
if /i "%~1"=="rebuild" set FORCE_BUILD=1

if "%FORCE_BUILD%"=="1" goto build
if exist "%EXE%" goto launch

echo [lanceur] executable introuvable : compilation du framework...
echo            (flutter build windows --debug, environ 1 a 2 min)

:build
echo [lanceur] build flutter windows --debug en cours...
cd /d "%ROOT%\flutter_app"
call flutter build windows --debug
if errorlevel 1 (
    echo [lanceur] ERREUR : le build flutter a echoue. Verifiez le message ci-dessus.
    popd
    pause
    exit /b 1
)

:launch
if not exist "%EXE%" (
    echo [lanceur] ERREUR : %EXE% introuvable apres build.
    popd
    pause
    exit /b 1
)
echo [lanceur] lancement de claps_studio.exe ...
start "" "%EXE%"
popd
endlocal
exit /b 0
