# Fjell

> Arbeidsnavn. En AI-assistert bildeoppryddingsapp.

Fjell gjør tusenvis av ukurerte bilder om til et bibliotek du faktisk vil åpne. Appen finner duplikater, serier og dårlige bilder, foreslår hva som kan slettes, og lar deg godkjenne alt på sekunder. **Den sletter aldri noe uten at du har sett det og kan angre.**

**Kjerneløftet:** «Frigjør 30 GB og 4 000 bilder på 10 minutter, uten å miste ett eneste bilde du bryr deg om.»

## Målgrupper
- **Hverdagsbrukeren:** 20 000+ bilder på mobilen, full lagring, ingen tid.
- **Familiefotografen:** flere enheter, delt bibliotek, mange nesten like bilder.
- **Hobby- og proffotografen:** culling av 2 000 RAW-filer etter en jobb.

## Dokumentasjon
| Fil | Innhold |
|---|---|
| [docs/PRODUKT.md](docs/PRODUKT.md) | Kjernefunksjoner og skjermer |
| [docs/TRYGGHET.md](docs/TRYGGHET.md) | Trygghetsmodell for sletting |
| [docs/TEKNIKK.md](docs/TEKNIKK.md) | Personvern, arkitektur, ytelse |
| [docs/INSPIRASJON.md](docs/INSPIRASJON.md) | Hva vi låner fra andre apper |
| [docs/VEIKART.md](docs/VEIKART.md) | Faser, forretningsmodell, suksessmål, risikoer |

## Struktur
- `core/` – indeksering, hashing, embeddings, scoring (delt logikk)
- `app/` – iOS og Android
- `desktop/` – Electron/Tauri for filbaserte biblioteker, NAS og eksterne disker
- `docs/` – spesifikasjon

## Status
Konseptfase. Ingen kode ennå.
