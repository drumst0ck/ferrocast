'use strict';
// Line-delimited JSON on stdout (runtime -> host) and the request/response table for host calls.
// console output is moved to stderr so extension logging can never corrupt the protocol.
const write = process.stdout.write.bind(process.stdout);
for (const k of ['log', 'info', 'debug', 'warn']) console[k] = (...a) => console.error(...a);

let nextId = 1;
const pending = new Map();

// A broken stdout/stderr (the host went away or stopped reading) ends the process: nothing may
// try to report that failure through the same broken pipe.
let broken = false;
function die() {
  broken = true;
  process.exit(1);
}
process.stdout.on('error', die);
process.stderr.on('error', die);

function log(text) {
  if (broken) return;
  try {
    process.stderr.write(text.endsWith('\n') ? text : text + '\n');
  } catch {
    die();
  }
}

function send(msg) {
  if (broken) return;
  try {
    write(JSON.stringify(msg) + '\n');
  } catch {
    die();
  }
}

function call(method, params) {
  return new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    send({ type: 'call', id, method, params });
  });
}

function result(msg) {
  const p = pending.get(msg.id);
  if (!p) return;
  pending.delete(msg.id);
  if (msg.error) p.reject(new Error(msg.error));
  else p.resolve(msg.value);
}

// set by main.js
const navigation = { push: () => {}, pop: () => {} };

module.exports = { send, call, result, navigation, log, die };
