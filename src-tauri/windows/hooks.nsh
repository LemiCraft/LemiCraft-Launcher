; Old launcher's own updater can't silently swap itself for this installer (different installer
; tech, no shared identity), so this removes the old install directly instead of running its
; uninstaller.exe — that shows a hardcoded Pascal MsgBox asking to delete {userappdata}\LemiCraft,
; and /SUPPRESSMSGBOXES does NOT suppress a script-authored one (confirmed live), which would hang
; unattended or risk deleting real account/mods data. Mirrors the old .iss's [Registry]/[Icons]
; entries exactly, never touching that folder.
!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Удаляю старую версию лаунчера..."

  ExecWait '"$SYSDIR\taskkill.exe" /F /IM "LemiCraft_Launcher.exe" /T'
  Sleep 500

  RMDir /r "$LOCALAPPDATA\Programs\LemiCraft Launcher"

  Delete "$SMPROGRAMS\LemiCraft Launcher\LemiCraft Launcher.lnk"
  Delete "$SMPROGRAMS\LemiCraft Launcher\Открыть папку игры.lnk"
  Delete "$SMPROGRAMS\LemiCraft Launcher\Удалить LemiCraft Launcher.lnk"
  RMDir "$SMPROGRAMS\LemiCraft Launcher"
  Delete "$DESKTOP\LemiCraft Launcher.lnk"
  Delete "$QUICKLAUNCH\LemiCraft Launcher.lnk"

  ; Inno's AppId escaping is asymmetric — "{{...}}" collapses the leading "{{" to one "{" but
  ; leaves the trailing "}}" as two literal closing braces, confirmed against the real registry key.
  ; lemicraft:// isn't touched here — the "Register deep links" step further down unconditionally
  ; rewrites it to the new install path regardless of what was here before.
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\{3E8F4A92-1B5C-4D7E-9F2A-8C3D5E6F7A8B}}_is1"
  DeleteRegKey HKCU "Software\LemiCraft\Launcher"
  DeleteRegKey /ifempty HKCU "Software\LemiCraft"
!macroend
