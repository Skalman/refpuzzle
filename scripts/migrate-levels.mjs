#!/usr/bin/env node
// Rekeys puzzle JSON files from level "0"-"5" to "1"-"6"
import { readFileSync, writeFileSync, readdirSync } from "fs";

const dir = "public/puzzles/daily";
for (const f of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
  const path = `${dir}/${f}`;
  const data = JSON.parse(readFileSync(path, "utf8"));
  const out = {};
  for (const [date, levels] of Object.entries(data)) {
    const newLevels = {};
    for (const [k, v] of Object.entries(levels)) {
      newLevels[String(Number(k) + 1)] = v;
    }
    out[date] = newLevels;
  }
  writeFileSync(path, JSON.stringify(out));
  console.log(`${f}: migrated ${Object.keys(data).length} days`);
}
