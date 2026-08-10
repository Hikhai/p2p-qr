; Đảm bảo WebView2Loader.dll nằm cạnh p2p-qr.exe (bắt buộc với toolchain windows-gnu).
!macro NSIS_HOOK_POSTINSTALL
  ; Resource map có thể đặt file ở $INSTDIR hoặc $INSTDIR\resources
  IfFileExists "$INSTDIR\WebView2Loader.dll" done_wv 0
  IfFileExists "$INSTDIR\resources\WebView2Loader.dll" 0 done_wv
    CopyFiles /SILENT "$INSTDIR\resources\WebView2Loader.dll" "$INSTDIR\WebView2Loader.dll"
  done_wv:
!macroend

!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
