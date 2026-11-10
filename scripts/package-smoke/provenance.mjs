// Downloads the fob-windows-x86_64 artifact of one Build Check or Draft
// Release run of this repository and proves it is the expected package.
// usage: node provenance.mjs <run id> <source sha> <zip sha256> <out dir>
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [runId, sourceSha, zipSha, out] = process.argv.slice(2);
const repo = process.env.GITHUB_REPOSITORY;
const token = process.env.GH_TOKEN;
const WORKFLOWS = [
  ".github/workflows/build-check.yml",
  ".github/workflows/release.yml",
];
const checks = [];
const check = (name, pass, detail) => checks.push({ name, pass, detail });
const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");
const api = async (path) => {
  const res = await fetch(`https://api.github.com/repos/${repo}/${path}`, {
    headers: {
      authorization: `Bearer ${token}`,
      accept: "application/vnd.github+json",
    },
  });
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return res;
};

mkdirSync(out, { recursive: true });
const report = { runId, expected: { sourceSha, zipSha }, checks };
try {
  check(
    "inputs are a run id, a full commit SHA and a SHA256",
    /^\d+$/.test(runId) &&
      /^[0-9a-f]{40}$/.test(sourceSha) &&
      /^[0-9a-f]{64}$/.test(zipSha),
  );
  const run = await (await api(`actions/runs/${runId}`)).json();
  report.run = {
    path: run.path,
    event: run.event,
    headBranch: run.head_branch,
    headSha: run.head_sha,
    headRepository: run.head_repository?.full_name,
    conclusion: run.conclusion,
    url: run.html_url,
  };
  check("run belongs to this repository", run.repository?.full_name === repo);
  check(
    "run built a commit of this repository, not a fork",
    run.head_repository?.full_name === repo,
    run.head_repository?.full_name,
  );
  check(
    "run is a trusted packaging workflow",
    WORKFLOWS.includes(run.path),
    run.path,
  );
  check(
    "run came from a push or a manual dispatch",
    ["push", "workflow_dispatch"].includes(run.event),
    run.event,
  );
  check("run succeeded", run.conclusion === "success", run.conclusion);
  check(
    "run built the expected commit",
    run.head_sha === sourceSha,
    run.head_sha,
  );

  const { artifacts } = await (
    await api(`actions/runs/${runId}/artifacts`)
  ).json();
  const artifact = artifacts.find((a) => a.name === "fob-windows-x86_64");
  check(
    "run has an unexpired fob-windows-x86_64 artifact",
    !!artifact && !artifact.expired,
  );
  report.artifact = artifact && {
    id: artifact.id,
    size: artifact.size_in_bytes,
    digest: artifact.digest,
    workflowRunHeadSha: artifact.workflow_run?.head_sha,
  };
  check(
    "artifact was uploaded by that commit",
    artifact?.workflow_run?.head_sha === sourceSha,
  );
  const archive = Buffer.from(
    await (await api(`actions/artifacts/${artifact.id}/zip`)).arrayBuffer(),
  );
  check(
    "artifact download matches GitHub's digest",
    artifact.digest === `sha256:${sha256(archive)}`,
    `sha256:${sha256(archive)}`,
  );
  const outer = join(out, "artifact.zip");
  writeFileSync(outer, archive);
  const unpacked = join(out, "artifact");
  mkdirSync(unpacked, { recursive: true });
  execFileSync("tar", ["-xf", outer, "-C", unpacked]);
  const zips = readdirSync(unpacked).filter((f) => f.endsWith(".zip"));
  check("artifact holds exactly one zip", zips.length === 1, zips);
  const zip = join(unpacked, zips[0]);
  report.zip = { name: zips[0], sha256: sha256(readFileSync(zip)) };
  check(
    "zip has the expected SHA256",
    report.zip.sha256 === zipSha,
    report.zip.sha256,
  );
  const app = join(out, "app");
  mkdirSync(app, { recursive: true });
  execFileSync("tar", ["-xf", zip, "-C", app]);
  const [top] = readdirSync(app);
  const dir = join(app, top);
  const sums = readFileSync(join(dir, "SHA256SUMS.txt"), "utf8")
    .trim()
    .split(/\r?\n/)
    .map((l) => l.split(/\s+\*?/, 2));
  const bad = sums.filter(
    ([sum, file]) => sha256(readFileSync(join(dir, file))) !== sum,
  );
  check(
    "every file in SHA256SUMS.txt matches",
    sums.length > 0 && !bad.length,
    bad.map(([, f]) => f),
  );
  const exe = sums.find(([, f]) => f === "fob.exe");
  check("SHA256SUMS.txt lists fob.exe", !!exe);
  report.fob = { dir, exeSha256: exe?.[0] };
} catch (error) {
  check("provenance completed", false, String(error));
}
report.pass = checks.every((c) => c.pass);
writeFileSync(join(out, "provenance.json"), JSON.stringify(report, null, 2));
for (const c of checks)
  console.log(
    `${c.pass ? "ok  " : "FAIL"} ${c.name}${c.detail === undefined ? "" : ` (${JSON.stringify(c.detail)})`}`,
  );
if (!report.pass) process.exit(1);
if (process.env.GITHUB_ENV)
  writeFileSync(
    process.env.GITHUB_ENV,
    `FOB_DIR=${report.fob.dir}\nFOB_EXE_SHA256=${report.fob.exeSha256}\n`,
    { flag: "a" },
  );
