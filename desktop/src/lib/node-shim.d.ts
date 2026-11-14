// Minimale typer for Node-API-ene som nonet.test.ts bruker (vitest kjører i Node; @types/node er ikke avhengighet i prosjektet).
declare module "node:fs" {
  export function readdirSync(path: string): string[];
  export function readFileSync(path: string, encoding: "utf8"): string;
  export function statSync(path: string): { isDirectory(): boolean };
  export function existsSync(path: string): boolean;
}
declare module "node:path" {
  export function dirname(p: string): string;
  export function join(...parts: string[]): string;
  export function relative(from: string, to: string): string;
}
declare module "node:url" {
  export function fileURLToPath(url: string | URL): string;
}
