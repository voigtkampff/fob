import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import {
  changelogSection,
  checkVersion,
  createManifest,
  packageName,
  projectRoot,
  stagePackage,
} from "./release.mjs";
import { uploadDraft } from "./upload-draft.mjs";

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), "fob-release-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, "src-tauri"));
  for (const file of [
    "package.json",
    "package-lock.json",
    "CHANGELOG.md",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
  ]) {
    await copyFile(join(projectRoot, file), join(root, file));
  }
  const version = await checkVersion(root);
  const artifacts = join(root, "artifacts");
  await mkdir(artifacts);
  await writeFile(join(artifacts, packageName(version)), "test zip");
  return { root, artifacts, version };
}

test("release versions must agree across both lockfiles and app manifests", async (t) => {
  const { root, version } = await fixture(t);
  assert.equal(await checkVersion(root, `v${version}`), version);
  await assert.rejects(checkVersion(root, "v99.0.0"), /does not match/);
  const path = join(root, "src-tauri/Cargo.lock");
  const lock = await readFile(path, "utf8");
  await writeFile(
    path,
    lock.replace(/(name = "winperf"\s*\nversion = ")[^"]+/, "$199.0.0"),
  );
  await assert.rejects(checkVersion(root), /Versions differ/);
});

test("the manifest hashes the one Windows zip", async (t) => {
  const { root, artifacts, version } = await fixture(t);
  const manifest = await createManifest(
    artifacts,
    `v${version}`,
    root,
    "abc123",
  );
  const digest = createHash("sha256").update("test zip").digest("hex");
  assert.deepEqual(manifest, {
    version,
    commit: "abc123",
    asset: {
      name: `fob-v${version}-x86_64-windows.zip`,
      size: 8,
      sha256: digest,
    },
  });
});

test("a missing, empty, or extra package blocks the release", async (t) => {
  const { root, artifacts, version } = await fixture(t);
  const path = join(artifacts, packageName(version));
  await writeFile(path, "");
  await assert.rejects(createManifest(artifacts, `v${version}`, root), /empty/);
  await rm(path);
  await assert.rejects(
    createManifest(artifacts, `v${version}`, root),
    /Missing/,
  );
  await writeFile(path, "restored");
  await writeFile(join(artifacts, "fob-v0.0.1-x86_64-windows.zip"), "stale");
  await assert.rejects(
    createManifest(artifacts, `v${version}`, root),
    /Unexpected release files: fob-v0.0.1/,
  );
});

test("staging copies the zip for the current version only", async (t) => {
  const { root, version } = await fixture(t);
  const source = join(root, "dist");
  await mkdir(source);
  await writeFile(join(source, "fob-v0.0.1-x86_64-windows.zip"), "old");
  const destination = join(root, "staged");
  await assert.rejects(stagePackage(source, destination, root), /Missing/);
  await writeFile(join(source, packageName(version)), "zip");
  assert.equal(
    await stagePackage(source, destination, root),
    packageName(version),
  );
  assert.equal(
    await readFile(join(destination, packageName(version)), "utf8"),
    "zip",
  );
});

test("release notes come from the CHANGELOG entry for the version", async (t) => {
  const { root, version } = await fixture(t);
  const section = await changelogSection(version, root);
  assert.ok(section.startsWith("- "));
  assert.ok(!section.includes(`## ${version}`));
  await assert.rejects(changelogSection("99.0.0", root), /no entry for 99.0.0/);
});

function githubFixture(tag, state = {}) {
  const writes = [];
  const context = {
    ref: `refs/tags/${tag}`,
    sha: "abc123",
    repo: { owner: "test", repo: "fob" },
  };
  const github = {
    rest: {
      repos: {
        getCommit: async () => ({ data: { sha: state.sha || context.sha } }),
        getReleaseByTag: async () => {
          if (state.release) return { data: state.release };
          throw Object.assign(new Error("not found"), { status: 404 });
        },
        listReleaseAssets: () => {},
        createRelease: async (data) => {
          writes.push(["create", data]);
          return { data: { id: 1, draft: true } };
        },
        updateRelease: async (data) => {
          writes.push(["update", data]);
          return { data: { id: data.release_id, draft: true } };
        },
        deleteReleaseAsset: async (data) => {
          writes.push(["delete", data]);
        },
        uploadReleaseAsset: async (data) => {
          writes.push(["upload", data]);
        },
      },
    },
    paginate: async () => state.assets || [],
  };
  return { github, context, writes };
}

test("draft upload refuses published releases and moved tags before any writes", async (t) => {
  const { root, artifacts, version } = await fixture(t);
  for (const state of [
    { release: { id: 1, draft: false } },
    { sha: "different-commit" },
  ]) {
    const { github, context, writes } = githubFixture(`v${version}`, state);
    await assert.rejects(
      uploadDraft({ github, context, directory: artifacts, root }),
      /published|no longer points/,
    );
    assert.deepEqual(writes, []);
  }
});

test("a new release is a Fob draft with the changelog and zip hash", async (t) => {
  const { root, artifacts, version } = await fixture(t);
  const { github, context, writes } = githubFixture(`v${version}`);
  await uploadDraft({ github, context, directory: artifacts, root });
  const create = writes.find(([kind]) => kind === "create")[1];
  assert.equal(create.draft, true);
  assert.equal(create.name, `Fob v${version}`);
  assert.equal(create.target_commitish, context.sha);
  assert.ok(create.body.includes(await changelogSection(version, root)));
  const digest = createHash("sha256").update("test zip").digest("hex");
  assert.ok(create.body.includes(`SHA256: \`${digest}\``));
  assert.ok(!/neohtop/i.test(create.body));
  const uploads = writes.filter(([kind]) => kind === "upload");
  assert.deepEqual(
    uploads.map(([, data]) => data.name),
    [packageName(version)],
  );
});

test("an interrupted draft upload can be retried without creating a second release", async (t) => {
  const { root, artifacts, version } = await fixture(t);
  const { github, context, writes } = githubFixture(`v${version}`, {
    release: { id: 5, draft: true },
    assets: [{ id: 7, name: packageName(version) }],
  });
  await uploadDraft({ github, context, directory: artifacts, root });
  assert.equal(writes.filter(([kind]) => kind === "create").length, 0);
  assert.equal(writes.find(([kind]) => kind === "update")[1].draft, true);
  assert.equal(writes.filter(([kind]) => kind === "delete").length, 1);
  assert.equal(writes.filter(([kind]) => kind === "upload").length, 1);
});

test("a draft with foreign assets, incomplete artifacts and non-tag runs cannot be written", async (t) => {
  const { root, artifacts, version } = await fixture(t);
  const foreign = githubFixture(`v${version}`, {
    release: { id: 5, draft: true },
    assets: [{ id: 8, name: "NeoHtop_setup.exe" }],
  });
  await assert.rejects(
    uploadDraft({ ...foreign, directory: artifacts, root }),
    /unexpected assets/,
  );
  assert.deepEqual(foreign.writes, []);
  const { github, context, writes } = githubFixture(`v${version}`);
  await rm(join(artifacts, packageName(version)));
  await assert.rejects(
    uploadDraft({ github, context, directory: artifacts, root }),
    /Missing/,
  );
  context.ref = "refs/heads/main";
  await assert.rejects(
    uploadDraft({ github, context, directory: artifacts, root }),
    /version tag/,
  );
  assert.deepEqual(writes, []);
});
