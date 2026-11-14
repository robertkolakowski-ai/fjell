// ICU-delmengde (PLAN-UX-I18N 5.1): enkle plassholdere, `plural`, `select` og `#` i plural.
// Samme delmengde som tools/gen-i18n godtar; alt annet feiler allerede ved generering.
// Meldinger parses én gang og caches.

type Node =
  | string
  | { k: "arg"; name: string }
  | { k: "hash" }
  | { k: "choice"; name: string; plural: boolean; branches: Map<string, Node[]> };

export type Arg = string | number;
export type Params = Readonly<Record<string, Arg>>;

function groupEnd(s: string, i: number): number {
  let depth = 0;
  for (let j = i; j < s.length; j++) {
    if (s[j] === "{") depth++;
    else if (s[j] === "}" && --depth === 0) return j;
  }
  throw new Error(`ubalansert { i «${s}»`);
}

function parse(msg: string, inPlural: boolean): Node[] {
  const out: Node[] = [];
  let lit = "";
  const flush = () => {
    if (lit) out.push(lit);
    lit = "";
  };
  for (let i = 0; i < msg.length; i++) {
    const c = msg[i];
    if (c === "#" && inPlural) {
      flush();
      out.push({ k: "hash" });
    } else if (c !== "{") {
      lit += c;
    } else {
      flush();
      const j = groupEnd(msg, i);
      const inner = msg.slice(i + 1, j);
      const first = inner.indexOf(",");
      if (first < 0) {
        out.push({ k: "arg", name: inner.trim() });
      } else {
        const name = inner.slice(0, first).trim();
        const rest = inner.slice(first + 1);
        const second = rest.indexOf(",");
        const kind = rest.slice(0, second).trim();
        if (kind !== "plural" && kind !== "select") throw new Error(`ikke støttet ICU-type «${kind}»`);
        const branches = new Map<string, Node[]>();
        const opts = rest.slice(second + 1);
        const re = /\s*([^\s{]+)\s*\{/y;
        for (let k = 0; ; ) {
          re.lastIndex = k;
          const m = re.exec(opts);
          if (!m) break;
          const open = m.index + m[0].length - 1;
          const close = groupEnd(opts, open);
          branches.set(m[1], parse(opts.slice(open + 1, close), kind === "plural"));
          k = close + 1;
        }
        out.push({ k: "choice", name, plural: kind === "plural", branches });
      }
      i = j;
    }
  }
  flush();
  return out;
}

const cache = new Map<string, Node[]>();

export interface FormatContext {
  /** BCP 47-tag for Intl (tall, flertallsregler). */
  intl: string;
}

function render(nodes: Node[], params: Params, ctx: FormatContext, hash: number | null, missing: (n: string) => void): string {
  let out = "";
  for (const n of nodes) {
    if (typeof n === "string") out += n;
    else if (n.k === "hash") out += new Intl.NumberFormat(ctx.intl).format(hash ?? 0);
    else if (n.k === "arg") {
      const v = params[n.name];
      if (v === undefined) {
        missing(n.name);
        out += `{${n.name}}`;
      } else out += typeof v === "number" ? new Intl.NumberFormat(ctx.intl).format(v) : v;
    } else {
      const v = params[n.name];
      if (v === undefined) {
        missing(n.name);
        out += `{${n.name}}`;
        continue;
      }
      let branch: Node[] | undefined;
      if (n.plural) {
        const num = Number(v);
        branch = n.branches.get(`=${num}`) ?? n.branches.get(new Intl.PluralRules(ctx.intl).select(num));
        branch ??= n.branches.get("other");
        out += render(branch ?? [], params, ctx, num, missing);
      } else {
        branch = n.branches.get(String(v)) ?? n.branches.get("other");
        out += render(branch ?? [], params, ctx, hash, missing);
      }
    }
  }
  return out;
}

/** Formater én melding. Manglende parameter gir synlig `{navn}` og kall til `onMissing` (test/logg). */
export function formatMessage(msg: string, params: Params, ctx: FormatContext, onMissing: (name: string) => void = () => {}): string {
  let ast = cache.get(msg);
  if (!ast) {
    ast = parse(msg, false);
    cache.set(msg, ast);
  }
  return render(ast, params, ctx, null, onMissing);
}
