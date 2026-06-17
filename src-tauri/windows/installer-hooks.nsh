; TaskCap - 安装/卸载时注册 taskcap:// 协议

!macro NSIS_HOOK_POSTINSTALL
  DeleteRegKey HKCU "Software\Classes\taskisland"
  WriteRegStr HKCU "Software\Classes\taskcap" "" "URL:taskcap Protocol"
  WriteRegStr HKCU "Software\Classes\taskcap" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\taskcap\shell\open\command" "" '"$INSTDIR\taskcap.exe" "%1"'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey HKCU "Software\Classes\taskcap"
  DeleteRegKey HKCU "Software\Classes\taskisland"
!macroend
