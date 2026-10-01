import { expect, test } from "bun:test";
import { checkBudget } from "../scripts/budget";

test("passes within limits and names what is over", () => {
  expect(checkBudget([{ label: "html", bytes: 10, limit: 20 }]).ok).toBe(true);
  const over = checkBudget([{ label: "css", bytes: 30, limit: 20 }, { label: "js", bytes: 1, limit: 5 }]);
  expect(over.ok).toBe(false);
  expect(over.lines.find((l) => l.includes("css"))).toContain("OVER");
});
