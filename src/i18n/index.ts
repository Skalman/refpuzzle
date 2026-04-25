import en from "./en.ts";

type Strings = typeof en;

export function t(): Strings {
  return en;
}
