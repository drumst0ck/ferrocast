"use strict";
// Test fixture: starts a long-running child process and reports its pid on stderr.
const { jsx } = require("react/jsx-runtime");
const { List } = require("@raycast/api");
const { spawn } = require("child_process");
function Command() {
  const c = spawn("sleep", ["60"], { stdio: "ignore" });
  console.error("CHILD_PID " + c.pid);
  return jsx(List, { children: jsx(List.Item, { id: "a", title: "x" }) });
}
module.exports = { default: Command };
