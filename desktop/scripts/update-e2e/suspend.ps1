# Suspenderer alle tråder i en prosess (NtSuspendProcess), slik at den «henger»: den svarer ikke på WM_CLOSE eller
# Restart Manager, men holder fortsatt exe-filen låst. Brukes av update.spec.ts for å etterligne en Fjell-instans som
# ikke lar seg lukke pent. Kun CI. Skriver «suspended <pid> <antall tråder>» ved suksess.
param([Parameter(Mandatory = $true)][int]$ProcessId)
$ErrorActionPreference = "Stop"
$nt = Add-Type -Name Nt -Namespace FjellE2e -PassThru -MemberDefinition '[DllImport("ntdll.dll")] public static extern int NtSuspendProcess(System.IntPtr handle);'
$p = Get-Process -Id $ProcessId
$rc = $nt::NtSuspendProcess($p.Handle)
if ($rc -ne 0) { throw ("NtSuspendProcess feilet for pid {0}: 0x{1:X8}" -f $ProcessId, $rc) }
Write-Output ("suspended {0} {1}" -f $ProcessId, $p.Threads.Count)
