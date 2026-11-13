# Hjelp i appen og støtteprosess (utkast)

Status: tekstutkast for i18n (nb/en). Ingen nøkler er lagt inn; nøkkelnavn under `help.*` er forslag og må følge `docs/mvp/PLAN-UX-I18N.md` del 5. Hurtigtaster hentes fra `desktop/src/shortcuts.ts` (eneste kilde; tabellen under er kopi av PLAN-UX 6.4 og må ikke endre den). Alle tekster må testes med `nb` (+15–30 %).

## Innholdsfortegnelse
1. Første gangs-guide · 2. Hurtigtaster (F1) · 3. Hva skjer med bildene mine? · 4. Gjenopprette fra papirkurv · 5. Skymapper · 6. Feilsøking · 7. Få hjelp (diagnoserapport)

## Tekstutkast
**help.start.title** nb: Kom i gang / en: Get started
- nb: 1. Velg mappene med bilder. 2. Trykk «Start skanning». 3. Se gjennom forslagene. 4. Flytt det du vil bli kvitt til papirkurven. Du kan angre.
- en: 1. Choose your photo folders. 2. Press "Start scan". 3. Review the suggestions. 4. Move what you don't want to the bin. You can undo.

**help.keys.title** nb: Hurtigtaster / en: Keyboard shortcuts. Innhold genereres fra tabellen: Ctrl+Z angre · Esc lukk · F1 hjelp · kortmodus ←/↑ eller Delete/→ (behold/papirkurv/hopp over) · Mellomrom zoom · Ctrl+A velg alle (Gjennomgang). Alle har knappealternativ.

**help.files.title** nb: Hva skjer med bildene mine? / en: What happens to my photos?
- nb: Fjell endrer aldri innholdet i bildene dine. Når du velger «Flytt til papirkurv», flyttes filen til en karantenemappe (`.fjell-karantene`) på samme disk. Ingenting sendes noe sted. Plassen er ikke frigjort før du tømmer papirkurven.
- en: Fjell never changes the contents of your photos. "Move to bin" moves the file to a quarantine folder (`.fjell-karantene`) on the same disk. Nothing is sent anywhere. Space is not freed until you empty the bin.

**help.restore.title** nb: Gjenopprette / en: Restore
- nb: Åpne Papirkurv, velg bildene og trykk «Gjenopprett». Rett etter en handling kan du også trykke «Angre» (Ctrl+Z). Bilder er gjenopprettbare til du tømmer eller fristen (standard 30 dager) går ut. Etter «Tøm» ligger de i Windows Papirkurv, med mindre du valgte permanent sletting.
- en: Open Bin, select the photos and press "Restore". Right after an action you can press "Undo" (Ctrl+Z). Photos can be restored until you empty the bin or the period (30 days by default) ends. After "Empty" they are in the Windows Recycle Bin unless you chose permanent deletion.

**help.cloud.title** nb: Skymapper / en: Cloud folders
- nb: Fjell kan skanne mapper som synkroniseres av OneDrive, Dropbox eller iCloud og vise forslag, men flytter ikke bilder til papirkurv der, så ikke sletting utløses i skyen. Gjenkjenningen bygger på mappenavnet. Filer som bare ligger i skyen lastes ikke ned.
- en: Fjell can scan folders synced by OneDrive, Dropbox or iCloud and show suggestions, but does not move photos to the bin there, so no deletion is triggered in the cloud. Detection is based on the folder name. Cloud-only files are not downloaded.

**help.trouble.title** nb: Feilsøking / en: Troubleshooting
| Symptom | nb | en |
|---|---|---|
| Bilde vises ikke | Formatet kan ikke vises ennå (f.eks. HEIC), eller filen er bare i skyen. Filen er ikke berørt. | Format not viewable yet (e.g. HEIC), or the file is cloud-only. The file is untouched. |
| «Merket, ikke flyttet» | Filen er låst eller blokkert (antivirus, annet program). Lukk programmet og prøv igjen. | The file is locked or blocked (antivirus, another app). Close it and try again. |
| Disk eller nettverksstasjon borte | Koble til igjen og skann på nytt. Bilder markeres ikke som slettet. | Reconnect and rescan. Photos are not marked deleted. |
| Windows-advarsel ved installasjon | Beta er usignert. «Mer info», «Kjør likevel». | The beta is unsigned. "More info", "Run anyway". |

## Støtteprosess
1. Bruker: Innstillinger → «Kopier diagnoserapport» (se `KRASJ-OG-DIAGNOSE.md`); limer teksten inn i e-post eller sak. Ingenting sendes av appen.
2. Støtte: les rapporten, be aldri om bilder, filnavn eller stier; be bare om beskrivelse i brukerens egne ord.
3. Sporing: sak med appversjon + feilkode fra rapporten; samme feilkode flere ganger → rettesak.
4. Kanal og responstid er ikke avklart (produkteier; e-postadresse/GitHub-saker). Personvern: ikke lagre rapporter lenger enn saken krever.
