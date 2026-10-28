"use strict";
// Test fixture: a List whose item pushes a Detail screen; the list also reacts to search text.
const { jsx } = require("react/jsx-runtime");
const React = require("react");
const { List, Detail, ActionPanel, Action, LocalStorage } = require("@raycast/api");

function Page() {
  return jsx(Detail, { markdown: "# Pushed\n\nhello", actions: jsx(ActionPanel, { children: jsx(Action.OpenInBrowser, { url: "https://example.test" }) }) });
}

function Command() {
  const [text, setText] = React.useState("");
  return jsx(List, {
    onSearchTextChange: setText,
    children: jsx(List.Item, {
      id: "one",
      title: "Item " + text,
      actions: jsx(ActionPanel, { children: jsx(Action.Push, { title: "Open page", target: jsx(Page, {}) }) }),
    }),
  });
}
module.exports = { default: Command };
