export interface FixtureFile { rel: string; sha256: string; role: "dup" | "serie" | "uskarp" | "skjermbilde" | "unik" | "korrupt"; group?: string }
export interface Manifest { seed: number; photosDir: string; dupGroups: number; files: FixtureFile[] }
export const DUP_GROUPS: number;
export function generate(root: string): Manifest;
