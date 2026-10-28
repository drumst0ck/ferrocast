"use strict";
// Test fixture: prints 5000 lines to stdout/stderr (console.log goes to stderr in the runtime) and renders a list.
const { jsx } = require("react/jsx-runtime");
const { List } = require("@raycast/api");
function Command() {
  for (let i = 0; i < 5000; i++) {
    console.log("chatty line " + i);
    console.error("chatty err " + i);
  }
  return jsx(List, { children: jsx(List.Item, { id: "a", title: "Still alive" }) });
}
module.exports = { default: Command };
