; gitid's additions to Tauri's NSIS installer (tauri.conf.json:
; bundle > windows > nsis > installerHooks).
;
; The installer puts the gitid command-line tool (gitid.exe) next to the app.
; These hooks add the install folder to the user's PATH on install and take it
; out again on uninstall, so `gitid` works in any new terminal, like VS Code's
; "Add to PATH" option.
;
; The app installs per user (Tauri's default installMode, under
; %LOCALAPPDATA%), so this is the user's PATH (HKCU\Environment), which needs
; no administrator rights. A perMachine install would need the system PATH.
;
; NSIS strings hold at most ${NSIS_MAX_STRLEN} characters. If the user's PATH
; can't be read in full, or would be too long with the folder added, it is
; left as it is rather than cut short; the app's Settings then says the
; command isn't on PATH. Values keep their type (REG_EXPAND_SZ), so entries
; like %USERPROFILE%\bin keep working.

!include LogicLib.nsh

!define GITID_ENV_KEY "Environment"
; Not taken from WinMessages.nsh, so this file doesn't depend on what the
; template includes.
!define GITID_HWND_BROADCAST 0xFFFF
!define GITID_WM_SETTINGCHANGE 0x001A

!macro NSIS_HOOK_POSTINSTALL
  Call gitidAddToPath
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Call un.gitidRemoveFromPath
!macroend

; Whether HKCU\Environment has a value named Path at all. Sets $R9 to 1 or 0.
!macro GITID_PATH_EXISTS
  StrCpy $R9 0
  StrCpy $R8 0
  ${Do}
    ClearErrors
    EnumRegValue $R7 HKCU "${GITID_ENV_KEY}" $R8
    ${If} ${Errors}
    ${OrIf} $R7 == ""
      ${ExitDo}
    ${EndIf}
    ${If} $R7 == "Path" ; StrCmp ignores case, like the registry
      StrCpy $R9 1
      ${ExitDo}
    ${EndIf}
    IntOp $R8 $R8 + 1
  ${Loop}
!macroend

!macro GITID_BROADCAST
  ; Tell Explorer (and so new terminals) that the environment changed.
  SendMessage ${GITID_HWND_BROADCAST} ${GITID_WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

Function gitidAddToPath
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4
  Push $R7
  Push $R8
  Push $R9

  ClearErrors
  ReadRegStr $R0 HKCU "${GITID_ENV_KEY}" "Path"
  ${If} ${Errors}
    !insertmacro GITID_PATH_EXISTS
    ${If} $R9 == 1
      ; There, but too long for NSIS (or not a string): don't touch it.
      DetailPrint "gitid: your PATH is too long to change safely; add $INSTDIR to it by hand."
      Goto done
    ${EndIf}
    StrCpy $R0 ""
  ${EndIf}

  ; Already there (with or without a trailing backslash)? Compare whole
  ; entries: ";a;b;" contains ";b;". StrCmp ignores case, like Windows paths.
  StrCpy $R1 ";$R0;"
  StrLen $R2 ";$INSTDIR;"
  StrCpy $R3 0
  ${Do}
    StrCpy $R4 $R1 $R2 $R3
    ${If} $R4 == ""
      ${ExitDo}
    ${EndIf}
    ${If} $R4 == ";$INSTDIR;"
      Goto done
    ${EndIf}
    IntOp $R3 $R3 + 1
  ${Loop}
  StrLen $R2 ";$INSTDIR\;"
  StrCpy $R3 0
  ${Do}
    StrCpy $R4 $R1 $R2 $R3
    ${If} $R4 == ""
      ${ExitDo}
    ${EndIf}
    ${If} $R4 == ";$INSTDIR\;"
      Goto done
    ${EndIf}
    IntOp $R3 $R3 + 1
  ${Loop}

  ; Leave room for the separator and NSIS's terminating NUL.
  StrLen $R2 $R0
  StrLen $R3 $INSTDIR
  IntOp $R2 $R2 + $R3
  IntOp $R2 $R2 + 2
  ${If} $R2 >= ${NSIS_MAX_STRLEN}
    DetailPrint "gitid: your PATH is too long to change safely; add $INSTDIR to it by hand."
    Goto done
  ${EndIf}

  ${If} $R0 == ""
    StrCpy $R0 "$INSTDIR"
  ${Else}
    ; Don't double the separator if PATH already ends with one.
    StrCpy $R1 $R0 1 -1
    ${If} $R1 == ";"
      StrCpy $R0 "$R0$INSTDIR"
    ${Else}
      StrCpy $R0 "$R0;$INSTDIR"
    ${EndIf}
  ${EndIf}
  WriteRegExpandStr HKCU "${GITID_ENV_KEY}" "Path" $R0
  DetailPrint "gitid: added $INSTDIR to your PATH."
  !insertmacro GITID_BROADCAST

  done:
  Pop $R9
  Pop $R8
  Pop $R7
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
FunctionEnd

Function un.gitidRemoveFromPath
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4
  Push $R5
  Push $R6

  ClearErrors
  ReadRegStr $R0 HKCU "${GITID_ENV_KEY}" "Path"
  ${If} ${Errors}
    ; Missing, or too long to rewrite safely: leave it.
    Goto done
  ${EndIf}

  ; Rebuild PATH entry by entry, leaving out the install folder (with or
  ; without a trailing backslash) and empty entries.
  StrCpy $R1 ""   ; result
  StrCpy $R2 ""   ; current entry
  StrCpy $R3 0    ; position
  StrCpy $R6 0    ; 1 once an entry was left out
  StrLen $R4 $R0
  ${Do}
    ${If} $R3 < $R4
      StrCpy $R5 $R0 1 $R3
    ${Else}
      StrCpy $R5 ";" ; end the last entry
    ${EndIf}
    ${If} $R5 == ";"
      ${If} $R2 == "$INSTDIR"
      ${OrIf} $R2 == "$INSTDIR\"
        StrCpy $R6 1
      ${ElseIf} $R2 != ""
        ${If} $R1 == ""
          StrCpy $R1 $R2
        ${Else}
          StrCpy $R1 "$R1;$R2"
        ${EndIf}
      ${EndIf}
      StrCpy $R2 ""
    ${Else}
      StrCpy $R2 "$R2$R5"
    ${EndIf}
    IntOp $R3 $R3 + 1
  ${LoopUntil} $R3 > $R4

  ${If} $R6 == 1
    ${If} $R1 == ""
      ; The installer created Path with only this folder in it.
      DeleteRegValue HKCU "${GITID_ENV_KEY}" "Path"
    ${Else}
      WriteRegExpandStr HKCU "${GITID_ENV_KEY}" "Path" $R1
    ${EndIf}
    !insertmacro GITID_BROADCAST
  ${EndIf}

  done:
  Pop $R6
  Pop $R5
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
FunctionEnd
