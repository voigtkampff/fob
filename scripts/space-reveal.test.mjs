import assert from "node:assert/strict";
import test from "node:test";
import { en, translate, zh } from "../src/lib/i18n/strings.js";
import { revealMessage } from "../src/lib/utils/space-reveal.js";

const t = (locale) => (key, vars) => translate(locale, key, vars);
const KEYS = [
  "space.revealGone",
  "space.revealDenied",
  "space.revealFailed",
  "space.revealFailedCode",
  "space.revealExplorer",
];

test("a missing item says the scan may be out of date and to rescan", () => {
  assert.equal(
    revealMessage({ kind: "gone" }, t("zh")),
    "此项已不存在，扫描结果可能已过时，按「重新扫描」更新",
  );
  assert.equal(
    revealMessage({ kind: "gone" }, t("en")),
    "This item no longer exists. The scan may be out of date, press Rescan to update it",
  );
  // The hint names the real button.
  assert.ok(zh["space.revealGone"].includes(`「${zh["space.rescan"]}」`));
  assert.ok(en["space.revealGone"].includes(en["space.rescan"]));
});

test("other failures do not claim the item is gone", () => {
  for (const locale of ["zh", "en"]) {
    const gone = revealMessage({ kind: "gone" }, t(locale));
    for (const error of [
      { kind: "denied" },
      { kind: "failed", code: 32 },
      { kind: "failed" },
      { kind: "explorer" },
    ]) {
      const text = revealMessage(error, t(locale));
      assert.notEqual(text, gone);
      assert.ok(!text.includes("不存在") && !/no longer exists/i.test(text));
      assert.ok(
        !text.includes("space."),
        `${locale} ${error.kind} untranslated`,
      );
    }
  }
  assert.equal(
    revealMessage({ kind: "failed", code: 32 }, t("zh")),
    "无法检查此项（错误 32），没有在资源管理器中显示",
  );
  assert.equal(
    revealMessage({ kind: "denied" }, t("en")),
    "Access to this item is denied, so it can't be shown in File Explorer",
  );
});

test("an unknown rejection is shown as it came", () => {
  assert.equal(revealMessage("boom", t("zh")), "boom");
  assert.equal(revealMessage(new Error("x"), t("en")), "Error: x");
});

test("the Chinese reveal strings use no full stop and the project terms", () => {
  for (const key of KEYS) {
    assert.ok(zh[key] && en[key], key);
    assert.ok(!zh[key].includes("。"), `zh ${key} has 。`);
  }
  for (const key of ["space.revealDenied", "space.revealExplorer"])
    assert.ok(
      zh[key].includes("资源管理器") && en[key].includes("File Explorer"),
    );
});
