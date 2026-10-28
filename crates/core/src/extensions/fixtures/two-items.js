"use strict";
// Test fixture: what Raycast's build produces for a tiny List command (CommonJS, automatic JSX runtime).
const { jsx, jsxs } = require("react/jsx-runtime");
const { List, ActionPanel, Action } = require("@raycast/api");

function Command() {
  return jsxs(List, {
    searchBarPlaceholder: "Fixture",
    children: [
      jsx(List.Item, {
        id: "alpha",
        title: "Alpha",
        subtitle: "first",
        accessories: [{ text: "A" }],
        actions: jsx(ActionPanel, {
          children: jsx(Action.CopyToClipboard, { title: "Copy Alpha", content: "alpha-value" }),
        }),
      }),
      jsx(List.Item, {
        id: "beta",
        title: "Beta",
        subtitle: "second",
        actions: jsx(ActionPanel, {
          children: jsx(Action.CopyToClipboard, { title: "Copy Beta", content: "beta-value" }),
        }),
      }),
    ],
  });
}
module.exports = { default: Command };
