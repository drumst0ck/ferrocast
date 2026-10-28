"use strict";
// Test fixture (no-view): reports the environment variable names it can see.
module.exports = { default: async function Command() { console.error("ENV " + Object.keys(process.env).join(",")); } };
