; Fjell: NSIS-hooker (tauri.conf.json -> bundle.windows.nsis.installerHooks). Se docs/OPPDATERING.md, «Når appen ikke lar seg lukke».
;
; Hvorfor: Tauris NSIS-mal sjekker med Restart Manager om $INSTDIR\fjell-desktop.exe er i bruk (makroen CheckIfAppIsRunning)
; og avbryter med «Kunne ikke avslutte Fjell» hvis RmShutdown feiler. Det har skjedd gjentatte ganger på en ekte maskin, både
; ved oppdatering fra appen og ved avinstallering. Malen setter inn NSIS_HOOK_PREINSTALL / NSIS_HOOK_PREUNINSTALL rett FØR
; CheckIfAppIsRunning (tauri-bundler 2.10.4, installer.nsi: Section Install og Section Uninstall), så vi avslutter gjenværende
; Fjell-prosesser her. Da finner Restart Manager normalt ingenting å lukke.
;
; Trygt å tvangsavslutte: Fjell har ingen ulagret tilstand. Indeksen er SQLite i WAL-modus (hver beslutning er lagret når den
; er tatt), og bildefiler skrives aldri, de flyttes bare. Et avbrutt skann fortsetter ved neste start.
;
; Bevisst UTEN «taskkill /T»: ved oppdatering fra appen er installeren et barn av fjell-desktop.exe (ShellExecute fra
; oppdateringspluginen). /T ville tatt med hele prosesstreet, altså installeren selv. WebView2-barna tas i stedet eksplisitt
; under, og bare de som tilhører Fjell (kommandolinjen peker på fjell-desktop.exe eller Fjells egen EBWebView-mappe).
; Andre apper røres aldri.
;
; Ingen feil hvis ingen prosess finnes, hvis taskkill/PowerShell mangler eller er sperret: hooken avbryter aldri installasjonen.
; Malens egen sjekk kjører uansett etterpå.

!include LogicLib.nsh

!macro FJELL_KILL_REMAINING
  Push $R8
  Push $R9
  ; Inntil 5 forsøk. taskkill: 0 = avsluttet noe (sjekk på nytt), 128 = ingen slik prosess (ferdig), annet = prøv igjen.
  ${ForEach} $R8 1 5 + 1
    nsExec::Exec /TIMEOUT=10000 '"$SYSDIR\taskkill.exe" /F /IM "${MAINBINARYNAME}.exe"'
    Pop $R9
    ${If} $R9 == "128"
    ${OrIf} $R9 == "error"
      ${ExitFor}
    ${EndIf}
    Sleep 500
  ${Next}
  ; Gjenværende WebView2-prosesser som tilhører Fjell (de avslutter normalt selv når verten dør). Beste forsøk, resultatet ignoreres.
  nsExec::Exec /TIMEOUT=20000 `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Get-CimInstance Win32_Process | Where-Object { $$_.Name -eq 'msedgewebview2.exe' -and ($$_.CommandLine -like '*--webview-exe-name=${MAINBINARYNAME}.exe*' -or $$_.CommandLine -like '*\${BUNDLEID}\EBWebView*') } | ForEach-Object { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue }"`
  Pop $R9
  ; Windows slipper fillåsen på exe-filen litt etter at prosessen er borte.
  Sleep 300
  Pop $R9
  Pop $R8
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro FJELL_KILL_REMAINING
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro FJELL_KILL_REMAINING
!macroend
