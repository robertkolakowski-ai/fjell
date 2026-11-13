# Overvåker prosesser og installervinduer mens appen oppdaterer seg selv (Windows PowerShell 5.1 / pwsh).
# Skriver endringer (prosess startet/avsluttet, kommandolinje, tekst i installervinduer via UI Automation) til loggfilen.
# Linjer som starter med «UIA» er vindustekst; testen leter etter feilvinduer der. Skjermbilde tas når et nytt vindu dukker opp.
param([Parameter(Mandatory = $true)][string]$Log, [int]$IntervalMs = 500)
$ErrorActionPreference = "SilentlyContinue"
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
$enc = New-Object System.Text.UTF8Encoding($false)
function W([string]$s) { [IO.File]::AppendAllText($Log, ("{0:HH:mm:ss.fff} {1}`n" -f (Get-Date), $s), $enc) }
function Shot([string]$name) {
  try {
    $b = [Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object Drawing.Bitmap $b.Width, $b.Height
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
    $bmp.Save($name, [Drawing.Imaging.ImageFormat]::Png); $g.Dispose(); $bmp.Dispose()
  } catch { W "skjermbilde feilet: $_" }
}
$known = @{}      # pid -> beskrivelse (for «startet»/«avsluttet»)
$uiaSeen = @{}    # signatur -> true
$shots = 0
W "watch.ps1 startet (intervall $IntervalMs ms)"
while ($true) {
  $now = @{}
  foreach ($p in Get-CimInstance Win32_Process) {
    $cl = [string]$p.CommandLine
    if ($p.Name -match 'fjell|setup|uninstall|nsis|webview2' -or $cl -match '/UPDATE') {
      $short = if ($cl.Length -gt 200) { $cl.Substring(0, 200) + "..." } else { $cl }
      $now[[int]$p.ProcessId] = "$($p.Name) ppid=$($p.ParentProcessId) cmd=$short"
    }
  }
  foreach ($k in $now.Keys) { if (-not $known.ContainsKey($k)) { W "+ pid=$k $($now[$k])" } }
  foreach ($k in @($known.Keys)) { if (-not $now.ContainsKey($k)) { W "- pid=$k $($known[$k])" } }
  $known = $now
  # Vinduer som eies av installer-lignende prosesser (ikke appen selv og ikke WebView2).
  foreach ($k in $now.Keys) {
    $d = $now[$k]
    if ($d -match '^(fjell-desktop|msedgewebview2)' -and $d -notmatch '/UPDATE') { continue }
    try {
      $cond = New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ProcessIdProperty, [int]$k)
      $wins = [Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children, $cond)
      foreach ($w in $wins) {
        $texts = @($w.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition) | ForEach-Object { $_.Current.Name } | Where-Object { $_ } | Select-Object -Unique)
        $line = "UIA pid=$k class=$($w.Current.ClassName) title='$($w.Current.Name)' texts=[$($texts -join ' | ')]"
        if (-not $uiaSeen.ContainsKey($line)) {
          $uiaSeen[$line] = $true; W $line
          if ($shots -lt 20) { $shots++; $png = ($Log -replace '\.log$', '') + "-vindu-$shots.png"; Shot $png }
        }
      }
    } catch { }
  }
  Start-Sleep -Milliseconds $IntervalMs
}
