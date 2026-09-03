import { Button } from "@cap/ui-solid";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import {
	createResource,
	createSignal,
	Match,
	onCleanup,
	Show,
	Switch,
} from "solid-js";
import toast from "solid-toast";
import { commands, events } from "~/utils/tauri";

type Progress = { downloaded: number; total: number | null };

export default function UpdateRequired() {
	const [status, { refetch }] = createResource(() =>
		commands.licensingStatus(),
	);
	const [progress, setProgress] = createSignal<Progress | null>(null);
	const [installed, setInstalled] = createSignal(false);
	const [busy, setBusy] = createSignal(false);

	const unlisten = events.updateDownloadProgress.listen((e) =>
		setProgress({
			downloaded: e.payload.downloaded,
			total: e.payload.total ?? null,
		}),
	);
	onCleanup(() => {
		void unlisten.then((stop) => stop());
	});

	const required = () => {
		const update = status()?.update;
		return update?.type === "updateRequired" ? update.minimum : null;
	};

	const install = async () => {
		setBusy(true);
		try {
			await commands.updatesDownloadAndInstall();
			setInstalled(true);
		} catch (e) {
			setProgress(null);
			const url = status()?.downloadUrl;
			toast.error(
				url
					? "The update could not install itself. Use the download link below."
					: `The update could not install itself: ${String(e)}`,
			);
		} finally {
			setBusy(false);
		}
	};

	const percent = () => {
		const p = progress();
		if (!p?.total) return null;
		return Math.min(Math.round((p.downloaded / p.total) * 100), 100);
	};

	return (
		<div class="flex flex-col gap-5 justify-center items-center p-8 h-full text-center">
			<Switch>
				<Match when={installed()}>
					<h1 class="text-base font-semibold text-gray-12">Update installed</h1>
					<p class="text-sm text-gray-10">
						Restart Shelf to finish. Your recordings and settings stay where
						they are.
					</p>
					<Button onClick={() => relaunch()}>Restart Shelf</Button>
				</Match>

				<Match when={required()}>
					{(minimum) => (
						<>
							<h1 class="text-base font-semibold text-gray-12">
								Shelf {minimum()} is required
							</h1>
							<p class="max-w-sm text-sm leading-relaxed text-gray-10">
								{status()?.message ??
									"This version can no longer be used. Install the update to carry on."}
							</p>
							<p class="text-xs text-gray-9">
								Installed: {status()?.currentVersion}
							</p>

							<Show when={percent() !== null}>
								<div class="w-full max-w-xs h-2 rounded-full bg-gray-3">
									<div
										class="h-2 rounded-full bg-blue-9"
										style={{ width: `${percent()}%` }}
									/>
								</div>
							</Show>

							<div class="flex flex-col gap-2 items-center">
								<Button disabled={busy()} onClick={install}>
									{busy() ? "Installing..." : "Install update"}
								</Button>
								<Show when={status()?.downloadUrl}>
									{(url) => (
										<button
											type="button"
											class="text-xs underline text-gray-10 hover:text-gray-12"
											onClick={() => void openUrl(url())}
										>
											Download it yourself instead
										</button>
									)}
								</Show>
							</div>
						</>
					)}
				</Match>

				<Match when={!required()}>
					<h1 class="text-base font-semibold text-gray-12">
						This version is fine
					</h1>
					<p class="max-w-sm text-sm text-gray-10">
						Nothing needs updating right now. You can close this window.
					</p>
					<Button variant="gray" onClick={() => void refetch()}>
						Check again
					</Button>
				</Match>
			</Switch>
		</div>
	);
}
