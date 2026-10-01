export interface BudgetEntry {
  label: string;
  bytes: number;
  limit: number;
}

export function checkBudget(entries: BudgetEntry[]): { ok: boolean; lines: string[] } {
  const kb = (n: number) => `${(n / 1000).toFixed(1)} KB`;
  const lines = entries.map(
    (e) => `${e.bytes <= e.limit ? "ok  " : "OVER"}  ${e.label.padEnd(30)} ${kb(e.bytes).padStart(9)} / ${kb(e.limit)}`,
  );
  return { ok: entries.every((e) => e.bytes <= e.limit), lines };
}
