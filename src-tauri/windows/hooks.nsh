; NSIS hooks for AClash.
; Real settings live in %APPDATA%\AClash (legacy: mihomo-lite).
; Only remove them when the uninstaller checkbox "Delete the application data"
; is checked ($DeleteAppDataCheckboxState = 1).

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'taskkill /F /T /IM "aclash.exe"'
  Pop $0
  nsExec::Exec 'taskkill /F /T /IM "mihomo.exe"'
  Pop $0
  ; Remove elevated logon task created for autostart.
  nsExec::Exec 'schtasks /Delete /TN "AClash" /F'
  Pop $0
  Sleep 800
!macroend

!macro NSIS_HOOK_POSTINSTALL
  SetShellVarContext current
  ; Point desktop shortcut at a dedicated .ico path so Explorer does not reuse
  ; the stale cache entry for aclash.exe from older installs.
  Delete "$DESKTOP\AClash.lnk"
  CreateShortCut "$DESKTOP\AClash.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\resources\aclash.ico" 0
  ; Same for Start Menu entry created by the installer.
  Delete "$SMPROGRAMS\AClash.lnk"
  CreateShortCut "$SMPROGRAMS\AClash.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\resources\aclash.ico" 0
  ; Refresh shell icon cache without killing Explorer.
  nsExec::Exec 'ie4uinit.exe -show'
  Pop $0
  System::Call 'shell32::SHChangeNotify(i 0x8000000, i 0, i 0, i 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Keep data when the installer is updating the same product.
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    Delete "$DESKTOP\AClash.lnk"
    Delete "$SMPROGRAMS\AClash.lnk"
    ; Honor the uninstaller checkbox — do not wipe settings by default.
    ${If} $DeleteAppDataCheckboxState = 1
      RmDir /r "$APPDATA\AClash"
      RmDir /r "$APPDATA\mihomo-lite"
      RmDir /r "$LOCALAPPDATA\AClash"
      RmDir /r "$LOCALAPPDATA\mihomo-lite"
      RmDir /r "$LOCALAPPDATA\com.aclash.app"
      RmDir /r "$LOCALAPPDATA\com.lianghuian.mihomo-lite"
      RmDir /r "$APPDATA\com.aclash.app"
      RmDir /r "$APPDATA\com.lianghuian.mihomo-lite"
    ${EndIf}
  ${EndIf}
!macroend
