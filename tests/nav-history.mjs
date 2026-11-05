import assert from "node:assert/strict";
import { emptyHistory, visit, step } from "../src/lib/utils/nav-history.js";

let h = emptyHistory();
let at = [];
const go = (to) => ((h = visit(h, at, to)), (at = to));
const move = (dir) => {
  const r = step(h, at, dir);
  if (r) ({ history: h, path: at } = r);
  return r;
};

assert.equal(move(-1), null, "nothing to go back to at the start");
go(["Users"]);
go(["Users", "me"]);
go(["Users", "me", "Videos"]);
move(-1);
assert.deepEqual(at, ["Users", "me"]);
move(-1);
assert.deepEqual(at, ["Users"]);
move(1);
assert.deepEqual(at, ["Users", "me"], "forward retraces the step back");
go(["Windows"]);
assert.equal(move(1), null, "a new folder drops the forward stack");
move(-1);
assert.deepEqual(at, ["Users", "me"]);
move(-1);
move(-1);
assert.deepEqual(at, [], "back reaches the drive root");
assert.equal(move(-1), null);
const before = h;
go([]);
assert.equal(h, before, "opening the folder you are in is not a visit");
console.log("PASS: back/forward history like Explorer");
