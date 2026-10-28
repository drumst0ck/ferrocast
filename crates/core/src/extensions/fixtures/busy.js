"use strict";
// Test fixture: renders, then blocks its event loop so it never reads stdin again.
const { jsx } = require("react/jsx-runtime");
const { List } = require("@raycast/api");
function Command() {
  setTimeout(() => { for (;;) {} }, 300);
  return jsx(List, { children: jsx(List.Item, { id: "a", title: "x" }) });
}
module.exports = { default: Command };
