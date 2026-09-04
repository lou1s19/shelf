// Checks that an update feed can actually be installed by the shipped app.
//
//   node scripts/verify-update-feed.mjs --feed <latest.json> --package <Shelf.app.tar.gz>
//   node scripts/verify-update-feed.mjs --feed https://example.de/updates/latest.json
//
// Why this exists: a feed signed with the wrong key, or pointing at a package
// that changed after signing, is refused silently by the updater. Nobody
// notices until months later, when the first update never arrives. So the
// release run proves the signature here instead of hoping.
//
// minisign format: base64 of algorithm ("Ed" raw, "ED" blake2b-512 prehash),
// then the 8-byte key id, then the payload. Node ships Ed25519 and blake2b512,
// so no dependency is needed. macOS ships LibreSSL, whose openssl(1) cannot do
// Ed25519 at all, which is why this is not a shell one-liner.

import { createHash, createPublicKey, verify } from "node:crypto";
import * as fs from "node:fs/promises";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const CONFIG = path.join(repo, "apps/desktop/src-tauri/tauri.prod.conf.json");

function arg(name, fallback) {
	const i = process.argv.indexOf(`--${name}`);
	if (i !== -1 && process.argv[i + 1]) return process.argv[i + 1];
	if (fallback !== undefined) return fallback;
	throw new Error(`missing --${name}`);
}

/** Strips the comment lines minisign puts above the payload. */
function payload(blockBase64) {
	const text = Buffer.from(blockBase64, "base64").toString("utf8");
	const line = text
		.split("\n")
		.find((l) => l && !l.startsWith("untrusted") && !l.startsWith("trusted"));
	if (!line) throw new Error("no payload line in the minisign block");
	return Buffer.from(line, "base64");
}

async function read(source) {
	if (/^https?:\/\//.test(source)) {
		const response = await fetch(source);
		if (!response.ok) {
			throw new Error(`${source} answered ${response.status}`);
		}
		return Buffer.from(await response.arrayBuffer());
	}
	return fs.readFile(source);
}

async function main() {
	const config = JSON.parse(await fs.readFile(CONFIG, "utf8"));
	const publicKey = payload(config.plugins.updater.pubkey);
	const feed = JSON.parse((await read(arg("feed"))).toString("utf8"));

	// Ed25519 wants a SPKI wrapper; the raw 32 bytes are the tail of the
	// minisign key, after the algorithm and the key id.
	const spki = Buffer.concat([
		Buffer.from("302a300506032b6570032100", "hex"),
		publicKey.subarray(10),
	]);
	const key = createPublicKey({ key: spki, format: "der", type: "spki" });

	let failures = 0;
	for (const [platform, entry] of Object.entries(feed.platforms)) {
		const signature = payload(entry.signature);
		const algorithm = signature.subarray(0, 2).toString();

		if (!publicKey.subarray(2, 10).equals(signature.subarray(2, 10))) {
			console.error(
				`${platform}: signed with a different key than the app trusts. ` +
					"The update would be refused.",
			);
			failures += 1;
			continue;
		}

		const pkg = await read(arg("package", entry.url));
		const signed =
			algorithm === "ED" ? createHash("blake2b512").update(pkg).digest() : pkg;

		if (verify(null, signed, key, signature.subarray(10))) {
			console.log(`${platform}: signature valid, ${pkg.length} bytes`);
		} else {
			console.error(
				`${platform}: signature does not match the package. It was probably ` +
					"rebuilt or re-uploaded after signing.",
			);
			failures += 1;
		}
	}

	if (failures > 0) process.exit(1);
	console.log(`feed for ${feed.version} is installable`);
}

main().catch((error) => {
	console.error(error.message);
	process.exit(1);
});
