#!/usr/bin/env node
// Minimal statisk HTTP-server for oppdateringstesten (bare 127.0.0.1). Serverer latest.json og setup.exe, og logger hver forespørsel.
// Bruk: node serve.mjs <rotmappe> <port> <loggfil>
import { appendFileSync, createReadStream, statSync } from "node:fs";
import http from "node:http";
import { join, normalize, resolve, sep } from "node:path";

const [rootArg, portArg, logFile] = process.argv.slice(2);
if (!rootArg || !portArg) { console.error("bruk: node serve.mjs <rotmappe> <port> [loggfil]"); process.exit(2); }
const root = resolve(rootArg);
const log = (s) => { const line = `${new Date().toISOString()} ${s}\n`; if (logFile) { try { appendFileSync(logFile, line); } catch { /* ignorer */ } } else process.stdout.write(line); };

const server = http.createServer((req, res) => {
  const done = (status, bytes) => log(`${req.method} ${req.url} -> ${status} (${bytes} byte)`);
  let file;
  try { file = normalize(join(root, decodeURIComponent(new URL(req.url ?? "/", "http://x").pathname))); } catch { res.writeHead(400).end(); return done(400, 0); }
  if (!file.startsWith(root + sep)) { res.writeHead(403).end(); return done(403, 0); }
  let st;
  try { st = statSync(file); if (!st.isFile()) throw new Error("ikke fil"); } catch { res.writeHead(404).end(); return done(404, 0); }
  res.writeHead(200, { "content-type": file.endsWith(".json") ? "application/json" : "application/octet-stream", "content-length": st.size });
  if (req.method === "HEAD") { res.end(); return done(200, 0); }
  createReadStream(file).on("error", () => res.destroy()).pipe(res);
  res.on("close", () => done(res.writableFinished ? 200 : 499, st.size));
});
server.listen(Number(portArg), "127.0.0.1", () => log(`lytter på 127.0.0.1:${portArg}, rot ${root}`));
