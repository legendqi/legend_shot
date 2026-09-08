Unicode True
!include "MUI2.nsh"

!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef SOURCE_EXE
  !error "SOURCE_EXE is required"
!endif
!ifndef OUTPUT_EXE
  !error "OUTPUT_EXE is required"
!endif
!ifndef ICON_FILE
  !error "ICON_FILE is required"
!endif

Name "Legend Shot"
OutFile "${OUTPUT_EXE}"
InstallDir "$LOCALAPPDATA\Programs\Legend Shot"
InstallDirRegKey HKCU "Software\LegendShot" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "${ICON_FILE}"
!define MUI_UNICON "${ICON_FILE}"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  File /oname=LegendShot.exe "${SOURCE_EXE}"

  CreateDirectory "$SMPROGRAMS\Legend Shot"
  CreateShortcut "$SMPROGRAMS\Legend Shot\Legend Shot.lnk" "$INSTDIR\LegendShot.exe"
  CreateShortcut "$DESKTOP\Legend Shot.lnk" "$INSTDIR\LegendShot.exe"

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\LegendShot" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "DisplayName" "Legend Shot"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\LegendShot.exe"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$DESKTOP\Legend Shot.lnk"
  Delete "$SMPROGRAMS\Legend Shot\Legend Shot.lnk"
  RMDir "$SMPROGRAMS\Legend Shot"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LegendShot"
  DeleteRegKey HKCU "Software\LegendShot"
SectionEnd
