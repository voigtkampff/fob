import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";

const repo = "voigtkampff/fob";
const shaPattern = /^[0-9a-f]{40}$/;
const execute = (command, args) =>
  execFileSync(command, args, {
    encoding: "utf8",
    timeout: 120000,
    stdio: ["ignore", "pipe", "pipe"],
  });

export function assertUtcCommit(raw) {
  const headers = raw.split("\n\n", 1)[0];
  for (const role of ["author", "committer"]) {
    const match = headers.match(
      new RegExp(`^${role} .+ <[^>]+> \\d+ ([+-]\\d{4})$`, "m"),
    );
    assert.equal(match?.[1], "+0000", `${role} metadata must use +0000`);
  }
}

export function mergeUtc(
  number,
  head,
  {
    gh = (args) => JSON.parse(execute("gh", ["api", ...args])),
    git = (args) => execute("git", args),
  } = {},
) {
  assert.match(
    String(number),
    /^[1-9]\d*$/,
    "A positive PR number is required",
  );
  assert.match(head, shaPattern, "An exact checked head SHA is required");
  const origin = git(["remote", "get-url", "origin"]).trim();
  assert.ok(
    [
      `https://github.com/${repo}.git`,
      `https://github.com/${repo}`,
      `git@github.com:${repo}.git`,
    ].includes(origin),
    "Run in the intended repository checkout",
  );
  const path = `repos/${repo}/pulls/${number}`;
  const pr = gh([path]);
  assert.ok(
    pr.state === "open" && !pr.draft && !pr.merged,
    "PR must be open and ready",
  );
  assert.equal(
    pr.base.ref,
    "main",
    "Only the established main PR flow is supported",
  );
  assert.equal(pr.head.sha, head, "PR head changed; review and check it again");
  assert.ok(
    pr.mergeable === true && pr.mergeable_state === "clean",
    "PR is not cleanly mergeable",
  );
  assert.match(pr.base.sha, shaPattern);
  const commits = git(["rev-list", `${pr.base.sha}..${head}`])
    .trim()
    .split(/\s+/);
  assert.ok(
    commits.length && commits.every((sha) => shaPattern.test(sha)),
    "Fetch the PR commits first",
  );
  for (const sha of commits) assertUtcCommit(git(["cat-file", "commit", sha]));
  const checks = gh([
    `repos/${repo}/commits/${head}/check-runs?filter=latest&per_page=100`,
  ]);
  assert.ok(
    checks.total_count <= 100,
    "Too many checks to validate in one page",
  );
  const source = checks.check_runs.filter(
    (check) => check.name === "Source checks",
  );
  assert.ok(
    source.length &&
      source.every(
        (check) =>
          check.head_sha === head &&
          check.status === "completed" &&
          check.conclusion === "success",
      ),
    "Source checks must pass on this exact head",
  );
  const result = gh([
    "--method",
    "PUT",
    `${path}/merge`,
    "--header",
    "Accept: application/vnd.github+json",
    "--header",
    "Time-Zone: Etc/UTC",
    "-f",
    "merge_method=squash",
    "-f",
    `sha=${head}`,
    "-f",
    `commit_title=${pr.title} (#${number})`,
  ]);
  assert.ok(
    result.merged && shaPattern.test(result.sha),
    "Provider did not confirm a merge",
  );
  try {
    git(["fetch", "origin", result.sha]);
    assertUtcCommit(git(["cat-file", "commit", result.sha]));
  } catch {
    throw new Error(
      `PR already merged as ${result.sha}; raw UTC verification failed. Stop without retrying or rewriting history.`,
    );
  }
  return result.sha;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    assert.equal(
      process.argv.length,
      4,
      "Usage: node scripts/merge-pr-utc.mjs PR_NUMBER CHECKED_HEAD_SHA",
    );
    console.log(
      `Merged with raw author and committer +0000: ${mergeUtc(process.argv[2], process.argv[3])}`,
    );
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
