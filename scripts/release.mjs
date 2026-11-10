import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { copyFile, lstat, mkdir, readFile, readdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const projectRoot = fileURLToPath(new URL("../", import.meta.url));

// scripts/package-windows.sh writes this zip into dist/.
export const packageName = (version) => `fob-v${version}-x86_64-windows.zip`;

export async function checkVersion(root = projectRoot, tag = "") {
  const pkg = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
  const npmLock = JSON.parse(
    await readFile(join(root, "package-lock.json"), "utf8"),
  );
  const config = JSON.parse(
    await readFile(join(root, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const cargo = await readFile(join(root, "src-tauri/Cargo.toml"), "utf8");
  const lock = await readFile(join(root, "src-tauri/Cargo.lock"), "utf8");
  const packageBlock = cargo
    .split(/(?=^\[)/m)
    .find((block) => block.startsWith("[package]"));
  const cargoVersion = packageBlock?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const crateName = packageBlock?.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
  const lockedVersion = lock
    .split(/(?=^\[\[package\]\])/m)
    .find((block) => block.match(/^name = "([^"]+)"/m)?.[1] === crateName)
    ?.match(/^version = "([^"]+)"/m)?.[1];
  const versions = [
    pkg.version,
    npmLock.version,
    npmLock.packages?.[""]?.version,
    config.version,
    cargoVersion,
    lockedVersion,
  ];
  if (
    !versions.every(
      (version) => typeof version === "string" && version === pkg.version,
    )
  ) {
    throw new Error(
      `Versions differ (package.json, npm lock/root package, Tauri, Cargo.toml, Cargo.lock): ${versions.join(", ")}`,
    );
  }
  if (
    !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/.test(
      pkg.version,
    )
  ) {
    throw new Error(`Invalid release version: ${pkg.version}`);
  }
  if (tag && tag !== `v${pkg.version}`) {
    throw new Error(`Tag ${tag} does not match v${pkg.version}`);
  }
  return pkg.version;
}

async function requirePackage(path) {
  const info = await lstat(path).catch(() => null);
  if (!info?.isFile() || info.size === 0)
    throw new Error(`Missing, empty, or non-file package: ${path}`);
  return info.size;
}

export async function stagePackage(source, destination, root = projectRoot) {
  const name = packageName(await checkVersion(root));
  await requirePackage(join(source, name));
  await mkdir(destination, { recursive: true });
  await copyFile(join(source, name), join(destination, name));
  return name;
}

async function sha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

export async function createManifest(
  directory,
  tag,
  root = projectRoot,
  commit = process.env.GITHUB_SHA || null,
) {
  const version = await checkVersion(root, tag);
  const name = packageName(version);
  const unexpected = (await readdir(directory)).filter((file) => file !== name);
  if (unexpected.length)
    throw new Error(`Unexpected release files: ${unexpected.join(", ")}`);
  const path = join(directory, name);
  const size = await requirePackage(path);
  return { version, commit, asset: { name, size, sha256: await sha256(path) } };
}

/** The CHANGELOG.md section for this version, without its heading. */
export async function changelogSection(version, root = projectRoot) {
  const changelog = await readFile(join(root, "CHANGELOG.md"), "utf8");
  const sections = changelog.split(/^## /m).slice(1);
  const section = sections.find(
    (text) => text.slice(0, text.indexOf("\n")).trim() === version,
  );
  const body = section?.slice(section.indexOf("\n")).trim();
  if (!body) throw new Error(`CHANGELOG.md has no entry for ${version}.`);
  return body;
}

export async function releaseNotes(manifest, root = projectRoot) {
  const { version, commit, asset } = manifest;
  return [
    `## ${version}`,
    "",
    await changelogSection(version, root),
    "",
    "## Verify the download",
    "",
    `- File: \`${asset.name}\``,
    `- SHA256: \`${asset.sha256}\``,
    "",
    `Built from commit ${commit}.`,
    "",
  ].join("\n");
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const [command, ...args] = process.argv.slice(2);
  try {
    switch (command) {
      case "check-version":
        console.log(
          `Release version: ${await checkVersion(projectRoot, args[0])}`,
        );
        break;
      case "stage":
        console.log(await stagePackage(...args));
        break;
      case "manifest":
        console.log(JSON.stringify(await createManifest(...args), null, 2));
        break;
      case "notes":
        console.log(await releaseNotes(await createManifest(...args)));
        break;
      default:
        throw new Error(
          "Usage: release.mjs check-version [tag] | stage <package-dir> <output-dir> | manifest <artifact-dir> <tag> | notes <artifact-dir> [tag]",
        );
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
