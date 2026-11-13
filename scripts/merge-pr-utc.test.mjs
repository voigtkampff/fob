import assert from "node:assert/strict";
import test from "node:test";
import { assertUtcCommit, mergeUtc } from "./merge-pr-utc.mjs";

const head = "a".repeat(40),
  base = "b".repeat(40),
  merged = "c".repeat(40);
const commit = (author = "+0000", committer = "+0000") =>
  `tree ${base}\nauthor anonymous <anonymous@example.invalid> 1700000000 ${author}\ncommitter anonymous <anonymous@example.invalid> 1700000001 ${committer}\n\nSynthetic fixture`;
function fixture() {
  const calls = [];
  const pr = {
    state: "open",
    draft: false,
    merged: false,
    title: "Use UTC merges",
    base: { ref: "main", sha: base },
    head: { sha: head },
    mergeable: true,
    mergeable_state: "clean",
  };
  const check = {
    name: "Source checks",
    head_sha: head,
    status: "completed",
    conclusion: "success",
  };
  const state = {
    pr,
    check,
    feature: commit(),
    result: commit(),
    failFetch: false,
  };
  const adapters = {
    gh(args) {
      calls.push(["gh", ...args]);
      if (args.includes("PUT")) return { merged: true, sha: merged };
      if (args[0].includes("check-runs"))
        return { total_count: 1, check_runs: [check] };
      return pr;
    },
    git(args) {
      calls.push(["git", ...args]);
      if (args[0] === "remote") return "https://github.com/voigtkampff/fob.git";
      if (args[0] === "rev-list") return head;
      if (args[0] === "cat-file")
        return args[2] === merged ? state.result : state.feature;
      if (state.failFetch) throw Error("Synthetic fetch failure");
      return "";
    },
  };
  return { calls, state, adapters };
}
const writes = (f) => f.calls.filter((call) => call.includes("PUT"));

test("merge sends a pinned squash request with explicit UTC and checks both raw fields", () => {
  const f = fixture();
  assert.equal(mergeUtc(33, head, f.adapters), merged);
  const request = writes(f)[0];
  assert.ok(request.includes("Time-Zone: Etc/UTC"));
  assert.ok(request.includes("merge_method=squash"));
  assert.ok(request.includes(`sha=${head}`));
  assert.equal(writes(f).length, 1);
  assert.ok(f.calls.some((call) => call[1] === "fetch" && call[3] === merged));
});

for (const [name, change] of [
  ["changed head", (s) => (s.pr.head.sha = base)],
  ["draft", (s) => (s.pr.draft = true)],
  ["wrong base", (s) => (s.pr.base.ref = "other")],
  ["unmergeable", (s) => (s.pr.mergeable = false)],
  ["pending checks", (s) => (s.check.status = "in_progress")],
  ["failed checks", (s) => (s.check.conclusion = "failure")],
  ["checks on another head", (s) => (s.check.head_sha = base)],
  ["missing Source checks", (s) => (s.check.name = "Other check")],
  ["non-UTC feature author", (s) => (s.feature = commit("+0100"))],
  ["non-UTC feature committer", (s) => (s.feature = commit("+0000", "-0100"))],
])
  test(`${name} is rejected before any merge`, () => {
    const f = fixture();
    change(f.state);
    assert.throws(() => mergeUtc(33, head, f.adapters));
    assert.equal(writes(f).length, 0);
  });

for (const [name, change] of [
  ["unexpected provider timezone", (s) => (s.result = commit("+0100"))],
  ["post-merge fetch failure", (s) => (s.failFetch = true)],
])
  test(`${name} explicitly reports an already completed merge and never retries`, () => {
    const f = fixture();
    change(f.state);
    assert.throws(
      () => mergeUtc(33, head, f.adapters),
      /already merged.*Stop without retrying/,
    );
    assert.equal(writes(f).length, 1);
  });

test("malformed input and missing raw metadata fail closed", () => {
  const f = fixture();
  assert.throws(() => mergeUtc("../33", head, f.adapters));
  assert.throws(() => mergeUtc(33, "HEAD", f.adapters));
  assert.throws(() =>
    assertUtcCommit("\n\nauthor anonymous <anonymous@example.invalid> 1 +0000"),
  );
  assert.equal(writes(f).length, 0);
});
