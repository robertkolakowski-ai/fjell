// Tall, størrelser, prosent og datoer formateres i UI med Intl (PLAN-UX-I18N 5.1, 2).
// Resultatet sendes som ferdig tekst inn i t(), f.eks. t("home.ring.caption", { size: formatBytes(n, intl) }).
// Intl gir riktig hardt mellomrom i nb («1,2 GB», «82 %»).

const UNITS = ["byte", "kilobyte", "megabyte", "gigabyte", "terabyte"] as const;

/** Bytes (u64 fra kjernen som number) -> «1,2 GB». Binærprefiks (1024) som i Windows Utforsker. */
export function formatBytes(bytes: number, intl: string): string {
  let v = Math.max(0, bytes);
  let u = 0;
  while (v >= 1024 && u < UNITS.length - 1) {
    v /= 1024;
    u++;
  }
  const text = new Intl.NumberFormat(intl, {
    style: "unit",
    unit: UNITS[u],
    unitDisplay: "short",
    maximumFractionDigits: u === 0 ? 0 : v < 10 ? 1 : 0,
  }).format(v);
  return text.replace(/\s/g, "\u00a0"); // hardt mellomrom mellom tall og enhet (også når Intl gir vanlig)
}

/** Andel 0..1 -> «82 %». */
export function formatPercent(fraction: number, intl: string): string {
  return new Intl.NumberFormat(intl, { style: "percent", maximumFractionDigits: 0 }).format(fraction);
}

export function formatNumber(n: number, intl: string): string {
  return new Intl.NumberFormat(intl).format(n);
}

/** Dato -> «14. juli 2025» / «14 July 2025». */
export function formatDate(d: Date | number, intl: string): string {
  return new Intl.DateTimeFormat(intl, { dateStyle: "long" }).format(d);
}
