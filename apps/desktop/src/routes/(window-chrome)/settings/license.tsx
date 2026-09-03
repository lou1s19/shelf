import { Button } from "@cap/ui-solid";
import { openUrl } from "@tauri-apps/plugin-opener";
import { createResource, createSignal, For, Show } from "solid-js";
import toast from "solid-toast";
import { commands } from "~/utils/tauri";
import { Section, SectionCard, SettingsPageContent } from "./Setting";

const FEATURE_NAMES: Record<string, string> = {
	app: "All of Shelf",
	"studio-recording": "Studio recording",
	"instant-recording": "Instant recording",
	screenshot: "Screenshots",
	"screenshot-editor": "Screenshot editor",
	teleprompter: "Teleprompter",
	captions: "Captions",
	ocr: "Text recognition",
	export: "Exporting",
};

function formatDate(seconds: number) {
	return new Date(seconds * 1000).toLocaleDateString();
}

export default function LicenseSettings() {
	const [status, { mutate, refetch }] = createResource(() =>
		commands.licensingStatus(),
	);
	const [key, setKey] = createSignal("");
	const [busy, setBusy] = createSignal(false);

	const activate = async () => {
		const value = key().trim();
		if (!value) return;

		setBusy(true);
		try {
			mutate(await commands.licensingActivate(value));
			setKey("");
			toast.success("License activated");
		} catch (e) {
			toast.error(String(e));
		} finally {
			setBusy(false);
		}
	};

	const remove = async () => {
		setBusy(true);
		try {
			mutate(await commands.licensingDeactivate());
			toast.success("License removed from this Mac");
		} finally {
			setBusy(false);
		}
	};

	const check = async () => {
		setBusy(true);
		try {
			mutate(await commands.licensingRefresh());
			toast.success("Checked");
		} catch (e) {
			toast.error(String(e));
			void refetch();
		} finally {
			setBusy(false);
		}
	};

	return (
		<SettingsPageContent>
			<Section
				title="License"
				description="Shelf checks license keys on this Mac. Nothing about you is sent anywhere."
			>
				<SectionCard padded class="space-y-3">
					<Show
						when={status()?.tier === "pro"}
						fallback={
							<p class="text-sm text-gray-11">
								<Show
									when={(status()?.lockedFeatures.length ?? 0) > 0}
									fallback="Shelf is free. There is nothing to unlock."
								>
									No license on this Mac.
								</Show>
							</p>
						}
					>
						<div class="space-y-1">
							<p class="text-sm text-gray-12">
								Licensed to {status()?.licensedTo}
							</p>
							<p class="text-xs text-gray-10">
								Order {status()?.licenseId}
								<Show when={status()?.licenseExpires}>
									{(expires) => <> · runs until {formatDate(expires())}</>}
								</Show>
							</p>
						</div>
						<Button variant="gray" size="sm" disabled={busy()} onClick={remove}>
							Remove license
						</Button>
					</Show>

					<Show when={status()?.tier !== "pro"}>
						<div class="flex gap-2 items-center">
							<input
								type="text"
								spellcheck={false}
								autocapitalize="off"
								autocomplete="off"
								placeholder="Paste your license key"
								value={key()}
								onInput={(e) => setKey(e.currentTarget.value)}
								class="flex-1 px-3 h-8 text-xs rounded-lg border border-gray-4 bg-gray-1 text-gray-12 placeholder:text-gray-9"
							/>
							<Button
								size="sm"
								disabled={busy() || !key().trim()}
								onClick={activate}
							>
								Activate
							</Button>
						</div>
						<Show when={status()?.buyUrl}>
							{(url) => (
								<button
									type="button"
									class="text-xs underline text-gray-10 hover:text-gray-12"
									onClick={() => void openUrl(url())}
								>
									Where to get a key
								</button>
							)}
						</Show>
					</Show>
				</SectionCard>
			</Section>

			<Show when={(status()?.lockedFeatures.length ?? 0) > 0}>
				<Section
					title="Needs a license"
					description="These parts stay locked until a key is activated."
				>
					<SectionCard class="divide-y divide-gray-3">
						<For each={status()?.lockedFeatures}>
							{(feature) => (
								<p class="px-4 py-3 text-sm text-gray-11">
									{FEATURE_NAMES[feature] ?? feature}
								</p>
							)}
						</For>
					</SectionCard>
				</Section>
			</Show>

			<Section
				title="Version check"
				description="Shelf asks the website once every few hours whether this version is still supported."
			>
				<SectionCard padded class="space-y-2">
					<p class="text-sm text-gray-11">
						Installed version {status()?.currentVersion}
					</p>
					<Show
						when={status()?.checksEnabled}
						fallback={
							<p class="text-xs text-gray-10">
								The check is switched off in this build.
							</p>
						}
					>
						<p class="text-xs text-gray-10">
							<Show
								when={status()?.lastChecked}
								fallback="Not checked on this Mac yet."
							>
								{(checked) => (
									<>
										Last checked {new Date(checked() * 1000).toLocaleString()}
									</>
								)}
							</Show>
						</p>
					</Show>
					<Show when={status()?.update.type === "updateSoon"}>
						<p class="text-xs text-amber-500">
							A newer version will be required soon.
						</p>
					</Show>
					<Button variant="gray" size="sm" disabled={busy()} onClick={check}>
						Check now
					</Button>
				</SectionCard>
			</Section>
		</SettingsPageContent>
	);
}
