"use strict";
// Test fixture: writes a 17 MB line to stdout.
const { jsx } = require("react/jsx-runtime");
const { List } = require("@raycast/api");
function Command() {
  process.stdout.write("x".repeat(17 * 1024 * 1024) + "\n");
  return jsx(List, {});
}
module.exports = { default: Command };
