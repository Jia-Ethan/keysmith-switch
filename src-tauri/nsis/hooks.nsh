; Keysmith Switch NSIS hooks.
; Refuse installing an older version over a newer one. User data in
; %USERPROFILE%\.keysmith-switch is never deleted on uninstall.

!include "WinVer.nsh"
!include "LogicLib.nsh"
!include "WordFunc.nsh"
!insertmacro VersionCompare

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" "DisplayVersion"
  ${If} $R0 != ""
    ${VersionCompare} $R0 "${VERSION}" $R1
    ; $R1 = 1 if $R0 > VERSION (existing is newer)
    ${If} $R1 == 1
      MessageBox MB_ICONSTOP "Keysmith Switch $R0 is already installed. This package is ${VERSION} and will not downgrade."
      Abort
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Keep ~/.keysmith-switch (prompts, history, backups). In-app "clear all data"
  ; is the only path that removes it.
!macroend
