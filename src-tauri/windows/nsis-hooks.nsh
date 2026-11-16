; BandwidthX NSIS hooks.
; The desktop IPC client is restricted to Administrators and LocalSystem,
; so the desktop app requests elevation and the installer is per-machine.

!macro NSIS_HOOK_PREINSTALL
  ; Remove an older BandwidthX service registration (including developer installs)
  ; before replacing the service executable.
  ExecWait '"$SYSDIR\sc.exe" stop BandwidthX' $0
  Sleep 1200
  ExecWait '"$SYSDIR\sc.exe" delete BandwidthX' $0
  Sleep 1200
!macroend

!macro NSIS_HOOK_POSTINSTALL
  IfFileExists "$INSTDIR\resources\bandwidthx-service.exe" 0 bw_service_binary_missing
  DetailPrint "Registering BandwidthX Windows enforcement service..."
  ExecWait '"$INSTDIR\resources\bandwidthx-service.exe" --install' $0
  IntCmp $0 0 bw_service_install_ok bw_service_install_failed bw_service_install_failed

bw_service_install_failed:
  MessageBox MB_ICONSTOP|MB_OK "BandwidthX could not register its Windows service (exit code $0). The application cannot enforce network policies until the service is installed."
  Abort

bw_service_install_ok:
  DetailPrint "Starting BandwidthX Windows enforcement service..."
  ExecWait '"$INSTDIR\resources\bandwidthx-service.exe" --start' $0
  IntCmp $0 0 bw_service_start_ok bw_service_start_failed bw_service_start_failed

bw_service_start_failed:
  ExecWait '"$SYSDIR\sc.exe" stop BandwidthX' $1
  ExecWait '"$SYSDIR\sc.exe" delete BandwidthX' $1
  MessageBox MB_ICONSTOP|MB_OK "BandwidthX installed, but its Windows service could not start (exit code $0). Reboot and reinstall, or consult the installation guide."
  Abort

bw_service_start_ok:
  Goto bw_service_install_done

bw_service_binary_missing:
  MessageBox MB_ICONSTOP|MB_OK "The BandwidthX service binary is missing from the installer. Installation cannot continue."
  Abort

bw_service_install_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Stop and remove the privileged service before NSIS deletes its executable.
  ExecWait '"$SYSDIR\sc.exe" stop BandwidthX' $0
  Sleep 1200
  ExecWait '"$SYSDIR\sc.exe" delete BandwidthX' $0
  Sleep 1200
!macroend
