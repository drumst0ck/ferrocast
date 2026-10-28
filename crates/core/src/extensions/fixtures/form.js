"use strict";
// Test fixture: a Form with a controlled field (upper-cases what is typed), a checkbox, a dropdown and
// a submit action that stores the values it receives through LocalStorage.
const { jsx, jsxs } = require("react/jsx-runtime");
const React = require("react");
const { Form, ActionPanel, Action, LocalStorage } = require("@raycast/api");

function Command() {
  const [name, setName] = React.useState("ann");
  return jsxs(Form, {
    actions: jsx(ActionPanel, {
      children: jsx(Action.SubmitForm, { title: "Save", onSubmit: (v) => LocalStorage.setItem("submitted", JSON.stringify({ ...v, whenLocal: v.when instanceof Date ? [v.when.getFullYear(), v.when.getMonth() + 1, v.when.getDate(), v.when.getHours(), v.when.getMinutes()].join(",") : null })) }),
    }),
    children: [
      jsx(Form.TextField, { id: "name", title: "Name", value: name, onChange: (v) => setName(v.toUpperCase()) }),
      jsx(Form.Checkbox, { id: "ok", label: "OK", defaultValue: true }),
      jsxs(Form.Dropdown, {
        id: "color",
        title: "Color",
        children: [jsx(Form.Dropdown.Item, { value: "r", title: "Red" }), jsx(Form.Dropdown.Item, { value: "g", title: "Green" })],
      }),
      jsx(Form.DatePicker, { id: "when", title: "When" }),
    ],
  });
}
module.exports = { default: Command };
