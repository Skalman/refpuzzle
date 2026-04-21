import { generate } from "../src/generator/assemble.ts";
import { profiles } from "../src/generator/difficulty.ts";
import { RNG } from "../src/generator/rng.ts";

const level = Number(process.argv[2] ?? 3);
const profile = profiles.find((p) => p.level === level)!;
const seeds = 5;
const times: number[] = [];

for (let s = 0; s < seeds; s++) {
	const rng = new RNG(42 + s * 7919);
	const t0 = performance.now();
	const result = generate(profile, rng);
	const elapsed = performance.now() - t0;
	times.push(elapsed);
	console.log(
		`seed ${s}: ${result ? "OK" : "FAIL"} ${elapsed.toFixed(0)}ms`,
	);
}

const avg = times.reduce((a, b) => a + b, 0) / times.length;
console.log(`\nL${level} avg: ${avg.toFixed(0)}ms over ${seeds} seeds`);
