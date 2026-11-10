import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { createManifest, projectRoot, releaseNotes } from "./release.mjs";

export async function uploadDraft({
  github,
  context,
  directory,
  root = projectRoot,
}) {
  if (!context.ref.startsWith("refs/tags/"))
    throw new Error("Draft releases require a version tag.");
  const tag = context.ref.slice("refs/tags/".length);
  // Validate everything before making any GitHub writes.
  const manifest = await createManifest(directory, tag, root, context.sha);
  const body = await releaseNotes(manifest, root);
  const { data: commit } = await github.rest.repos.getCommit({
    ...context.repo,
    ref: tag,
  });
  if (commit.sha !== context.sha)
    throw new Error(
      "The version tag no longer points to the commit built by this workflow.",
    );

  let release;
  try {
    ({ data: release } = await github.rest.repos.getReleaseByTag({
      ...context.repo,
      tag,
    }));
  } catch (error) {
    if (error.status !== 404) throw error;
  }
  if (release && !release.draft)
    throw new Error("Refusing to modify an already published release.");

  const { name } = manifest.asset;
  const existing = release
    ? await github.paginate(github.rest.repos.listReleaseAssets, {
        ...context.repo,
        release_id: release.id,
        per_page: 100,
      })
    : [];
  if (existing.some((asset) => asset.name !== name)) {
    throw new Error(
      "The existing draft has unexpected assets. Review them before retrying this release.",
    );
  }
  const fields = {
    name: `Fob v${manifest.version}`,
    body,
    draft: true,
    prerelease: manifest.version.includes("-"),
  };
  if (release) {
    ({ data: release } = await github.rest.repos.updateRelease({
      ...context.repo,
      release_id: release.id,
      ...fields,
    }));
  } else {
    ({ data: release } = await github.rest.repos.createRelease({
      ...context.repo,
      tag_name: tag,
      target_commitish: context.sha,
      ...fields,
    }));
  }
  for (const previous of existing)
    await github.rest.repos.deleteReleaseAsset({
      ...context.repo,
      asset_id: previous.id,
    });
  const data = await readFile(join(directory, name));
  await github.rest.repos.uploadReleaseAsset({
    ...context.repo,
    release_id: release.id,
    name,
    data,
    headers: {
      "content-type": "application/zip",
      "content-length": data.length,
    },
  });
  return release.html_url;
}
