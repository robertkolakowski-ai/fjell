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
;
; Logg (bare diagnose, endrer ikke oppførselen): hooken skriver %TEMP%\fjell-installer.log med fase, taskkill-koder per forsøk,
; om installeren kjører som administrator, og hvor mange fjell-desktop.exe-prosesser som lever før og etter. Ingen stier, brukernavn
; eller filnavn loggføres, og fila forlater aldri maskinen (Fjell leser den ikke engang). Den kuttes ved 100 KB. Den er til for å
; forklare «Kunne ikke avslutte Fjell» på en ekte maskin, se docs/OPPDATERING.md og docs/WINDOWS-TESTMASKIN.md.

!include LogicLib.nsh

; Legger en linje på slutten av %TEMP%\fjell-installer.log (kun ASCII i loggteksten, så fila leses likt uansett kodeside). Feiler aldri (skrivefeil ignoreres) og lar feilflagget stå urørt for malen.
!macro FJELL_LOG TEXT
  Push $R6
  Push $R7
  ClearErrors
  FileOpen $R7 "$TEMP\fjell-installer.log" r
  ${IfNot} ${Errors}
    FileSeek $R7 0 END $R6
    FileClose $R7
    ${If} $R6 > 102400
      Delete "$TEMP\fjell-installer.log"
    ${EndIf}
  ${EndIf}
  ClearErrors
  FileOpen $R7 "$TEMP\fjell-installer.log" a
  ${IfNot} ${Errors}
    FileSeek $R7 0 END
    FileWrite $R7 "${TEXT}$\r$\n"
    FileClose $R7
  ${EndIf}
  ClearErrors
  Pop $R7
  Pop $R6
!macroend

; Antall fjell-desktop.exe-prosesser som én linje i loggen (`$$` er NSIS-escape for `$` i PowerShell-kommandoene under).
!macro FJELL_LOG_PROCS LABEL
  Push $R6
  Push $R7
  nsExec::ExecToStack /TIMEOUT=15000 `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "(Get-Process -Name '${MAINBINARYNAME}' -ErrorAction SilentlyContinue | Measure-Object).Count"`
  Pop $R6
  Pop $R7
  ${If} $R6 == "error"
  ${OrIf} $R6 == "timeout"
    !insertmacro FJELL_LOG "${LABEL}: prosesstelling feilet ($R6)"
  ${Else}
    !insertmacro FJELL_LOG "${LABEL}: prosesser av ${MAINBINARYNAME}.exe = $R7"
  ${EndIf}
  Pop $R7
  Pop $R6
!macroend

!macro FJELL_KILL_REMAINING
  Push $R8
  Push $R9
  Push $R5
  Push $R4
  ; Administrator? (Hooken hjelper ikke hvis Fjell kjører forhøyet og installeren ikke gjør det, eller omvendt.)
  nsExec::ExecToStack /TIMEOUT=15000 `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)"`
  Pop $R5
  Pop $R4
  ${If} $R5 == "error"
  ${OrIf} $R5 == "timeout"
    !insertmacro FJELL_LOG "installer: administrator ukjent ($R5)"
  ${Else}
    !insertmacro FJELL_LOG "installer: administrator = $R4"
  ${EndIf}
  !insertmacro FJELL_LOG_PROCS "for"
  ; Inntil 5 forsøk. taskkill: 0 = avsluttet noe (sjekk på nytt), 128 = ingen slik prosess (ferdig), annet = prøv igjen.
  ${ForEach} $R8 1 5 + 1
    nsExec::Exec /TIMEOUT=10000 '"$SYSDIR\taskkill.exe" /F /IM "${MAINBINARYNAME}.exe"'
    Pop $R9
    !insertmacro FJELL_LOG "taskkill forsok $R8: kode $R9"
    ${If} $R9 == "128"
    ${OrIf} $R9 == "error"
      ${ExitFor}
    ${EndIf}
    Sleep 500
  ${Next}
  ; Gjenværende WebView2-prosesser som tilhører Fjell (de avslutter normalt selv når verten dør). Beste forsøk, resultatet ignoreres.
  nsExec::Exec /TIMEOUT=20000 `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Get-CimInstance Win32_Process | Where-Object { $$_.Name -eq 'msedgewebview2.exe' -and ($$_.CommandLine -like '*--webview-exe-name=${MAINBINARYNAME}.exe*' -or $$_.CommandLine -like '*\${BUNDLEID}\EBWebView*') } | ForEach-Object { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue }"`
  Pop $R9
  !insertmacro FJELL_LOG "webview2-opprydding: kode $R9"
  ; Windows slipper fillåsen på exe-filen litt etter at prosessen er borte.
  Sleep 300
  !insertmacro FJELL_LOG_PROCS "etter"
  Pop $R4
  Pop $R5
  Pop $R9
  Pop $R8
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro FJELL_LOG "== fase: installasjon (pre)"
  !insertmacro FJELL_KILL_REMAINING
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro FJELL_LOG "== fase: avinstallasjon (pre)"
  !insertmacro FJELL_KILL_REMAINING
!macroend
