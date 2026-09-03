// Turns a finished release bundle into the two files the website serves:
// updates/latest.json (what the app asks for) and the download link target.
//
//   node scripts/make-update-feed.mjs --version 1.0.0 --base-url https://example.de
//
// Reads the signature Tauri produced next to the .app.tar.gz. Without that
// signature the app refuses the update, so a missing .sig stops us here rather
// than at the user's machine.

import * as fs from "node:fs/promises";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");

function arg(name, fallback) {
	const i = process.argv.indexOf(`--${name}`);
	if (i !== -1 && process.argv[i + 1]) return process.argv[i + 1];
	if (fallback !== undefined) return fallback;
	throw new Error(`missing --${name}`);
}

async function main() {
	const version = arg("version");
	const baseUrl = arg("base-url").replace(/\/+$/, "");
	// Where the package itself is downloaded from, when that is not the website.
	// A GitHub release asset lives under a flat /releases/download/<tag>/ path,
	// nothing like the /updates/<version>/ layout the website uses, so the two
	// cannot be derived from one another.
	const assetBase = arg("asset-base", "").replace(/\/+$/, "");
	const notes = arg("notes", "");
	const bundleDir = arg(
		"bundle-dir",
		path.join(repo, "target/release/bundle/macos"),
	);
	const outDir = arg("out", path.join(repo, "target/release-feed"));

	const entries = await fs.readdir(bundleDir);
	const tarball = entries.find((name) => name.endsWith(".app.tar.gz"));
	if (!tarball) {
		throw new Error(
			`no .app.tar.gz in ${bundleDir}. Build with createUpdaterArtifacts enabled ` +
				"(tauri.prod.conf.json) and sign it with TAURI_SIGNING_PRIVATE_KEY set.",
		);
	}

	const signature = (
		await fs.readFile(path.join(bundleDir, `${tarball}.sig`), "utf8")
	).trim();
	if (!signature) throw new Error(`${tarball}.sig is empty`);

	// One entry per architecture this bundle actually runs on. Listing an
	// architecture the build does not cover hands those Macs a broken update.
	const platforms = arg("platforms", "darwin-aarch64")
		.split(",")
		.map((name) => name.trim())
		.filter(Boolean);

	const feed = {
		version,
		notes,
		pub_date: new Date().toISOString(),
		platforms: Object.fromEntries(
			platforms.map((name) => [
				name,
				{
					signature,
					url: assetBase
						? `${assetBase}/${tarball}`
						: `${baseUrl}/updates/${version}/${tarball}`,
				},
			]),
		),
	};

	await fs.mkdir(path.join(outDir, "updates", version), { recursive: true });
	await fs.writeFile(
		path.join(outDir, "updates", "latest.json"),
		`${JSON.stringify(feed, null, 2)}\n`,
	);
	// Copied even when the package is served elsewhere: the signature in
	// latest.json covers this exact file, so whatever gets uploaded has to be
	// byte for byte this one.
	await fs.copyFile(
		path.join(bundleDir, tarball),
		path.join(outDir, "updates", version, tarball),
	);

	console.log(`feed written to ${outDir}`);
	console.log(`  updates/latest.json`);
	console.log(`  updates/${version}/${tarball}`);
	console.log();
	console.log("Upload both, keeping the paths, then check that");
	console.log(`  ${baseUrl}/updates/latest.json`);
	console.log("answers with JSON and not with an HTML error page.");
}

main().catch((error) => {
	console.error(error.message);
	process.exit(1);
});
