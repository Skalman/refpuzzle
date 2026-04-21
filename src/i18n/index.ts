import en from "./en.ts";

type Strings = typeof en;

const locales: Record<string, Strings> = { en };

let current: Strings = en;

export function setLocale(locale: string) {
	const strings = locales[locale];
	if (strings) current = strings;
}

export function t(): Strings {
	return current;
}
