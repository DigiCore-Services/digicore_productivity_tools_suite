MSI vs NSIS

# MSI (Microsoft Installer) and      NSIS (Nullsoft Scriptable Install      System)   are both popular tools for

## MSI is a  Microsoft-developed      installation package format that      uses a  database-driven approach. It

## packages    are transactional, meaning installations can be rolled      back  if something fails, and they



## NSIS, on   the other  hand, is an  open-source, script-based installer system originally created by
## Nullsoft.  It allows developers   to write . NSI scripts that define      every  aspect  of the installation

## small EXE installers, and supports extensive customization through plugins      and scripting logic.

priorities.
Example    NSIS Script:

  utFi1e "Mylnstaller.exe"
  Section
    Setout-Path     "$INSTDIR"
    File   "AppFinder.exe"
    CreateShortcut "$SMPROGRAMS\My Program. Ink " "$1NSTDIR\AppFinder.exe"
  SectionEnd

This script installs AppFinder.exe and creates a Start Menu      shortcut. It's compiled with
 makensis.exe into a distributable EXE.