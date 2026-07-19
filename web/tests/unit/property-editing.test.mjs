import test from "node:test";
import assert from "node:assert/strict";
import {
  applyMultiSelectDelta,
  checkboxIsChecked,
  isMultiSelectType,
  multiSelectDelta,
  parseMultiSelectValue,
  propertyLinkTarget,
  toggledCheckboxValue,
} from "../../src/lib/property-editing.ts";

test("multi-select uses independent add/remove members", () => {
  assert.deepEqual(parseMultiSelectValue("alpha, beta, alpha"), ["alpha", "beta"]);
  assert.deepEqual(multiSelectDelta("alpha, beta", ["beta", "gamma"]), {
    current: ["alpha", "beta"],
    add: ["gamma"],
    remove: ["alpha"],
  });
  assert.equal(applyMultiSelectDelta("alpha, beta", ["gamma"], ["alpha"]), "beta, gamma");
  assert.equal(isMultiSelectType("multi-select"), true);
  assert.equal(isMultiSelectType("multiselect"), true);
});

test("checkbox toggle is canonical and case-insensitive", () => {
  assert.equal(checkboxIsChecked("TRUE"), true);
  assert.equal(toggledCheckboxValue("true"), "false");
  assert.equal(toggledCheckboxValue("false"), "true");
});

test("property links normalize safe url, mailto, and tel targets", () => {
  assert.equal(propertyLinkTarget("url", "example.com/path"), "https://example.com/path");
  assert.equal(propertyLinkTarget("url", "javascript:alert(1)"), null);
  assert.equal(propertyLinkTarget("email", "hello@example.com"), "mailto:hello@example.com");
  assert.equal(propertyLinkTarget("phone", "+1 (312) 555-0199"), "tel:+13125550199");
});
