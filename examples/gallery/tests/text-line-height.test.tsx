// Line height: a unitless multiple (`leading-[1.72]`), the same as points
// and the font's own all lay out every line. A multiple read as points is
// 1.72pt: a block of paragraphs collapses and draws nothing
// (longbridge/nsh#289).
//
// The regression test is `tailwind::tests::leading_arbitrary` in nsh-core;
// this one only walks the gallery's card, so the demo a user sees stays
// wired to the three notations.
import { expect, open, test } from "nsh/test";

const CODE = "const raf = requestAnimationFrame;\nif (raf) run();\nelse fallback();";

test("the Line height card shows the block in all three notations", async () => {
  const app = await open("../App.tsx", { clock: {} });
  await app.press("label:Text");
  await app.waitFor("Line height");
  expect(app.texts().filter((t: string) => t === CODE).length).toBe(3);
});
