// Small presentation helpers shared across the public web app.

/**
 * Return `word` pluralized for `count` — English "add s" plus a few common
 * irregulars. Returns the noun only (not the count) so callers can style the
 * number separately, e.g. `{n} {plural(n, "vote")}` or
 * `<strong>{n}</strong> {plural(n, "vote")}`.
 */
export function plural(count: number, word: string, pluralForm?: string): string {
  if (count === 1) return word;
  if (pluralForm) return pluralForm;
  if (/[^aeiou]y$/i.test(word)) return `${word.slice(0, -1)}ies`;
  if (/(s|x|z|ch|sh)$/i.test(word)) return `${word}es`;
  return `${word}s`;
}

/** "1 vote" / "3 votes" — count and pluralized noun together. */
export function countLabel(count: number, word: string, pluralForm?: string): string {
  return `${count} ${plural(count, word, pluralForm)}`;
}
