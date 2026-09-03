import { A, type RouteSectionProps, useNavigate } from "@solidjs/router";
import { getVersion } from "@tauri-apps/api/app";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import * as dialog from "@tauri-apps/plugin-dialog";
import { createSignal, For, onMount, Show, Suspense } from "solid-js";
import toast from "solid-toast";
import { CapErrorBoundary } from "~/components/CapErrorBoundary";
import { commands } from "~/utils/tauri";
import IconLucideKeyRound from "~icons/lucide/key-round";
import IconLucideTerminal from "~icons/lucide/terminal";
import IconLucideVideo from "~icons/lucide/video";
import IconLucideZap from "~icons/lucide/zap";

function SettingsContentSkeleton() {
	return (
		<div class="cap-settings-page flex flex-col h-full custom-scroll">
			<div class="px-6 py-6 space-y-7 max-w-[42rem]" aria-hidden="true">
				<div class="space-y-2.5">
					<div class="px-1 space-y-1.5">
						<div class="h-4 w-28 rounded-full bg-gray-4 animate-pulse" />
						<div class="h-3 w-72 max-w-full rounded-full bg-gray-4 animate-pulse" />
					</div>
					<div class="cap-settings-card overflow-hidden rounded-xl border border-gray-3 bg-gray-2 divide-y divide-gray-3">
						<div class="px-4 py-3.5 space-y-2">
							<div class="h-[15px] w-40 rounded-full bg-gray-4 animate-pulse" />
							<div class="h-3 w-64 max-w-full rounded-full bg-gray-4 animate-pulse" />
						</div>
						<div class="px-4 py-3.5 space-y-2">
							<div class="h-[15px] w-36 rounded-full bg-gray-4 animate-pulse" />
							<div class="h-3 w-56 max-w-full rounded-full bg-gray-4 animate-pulse" />
						</div>
						<div class="px-4 py-3.5 space-y-2">
							<div class="h-[15px] w-44 rounded-full bg-gray-4 animate-pulse" />
							<div class="h-3 w-60 max-w-full rounded-full bg-gray-4 animate-pulse" />
						</div>
					</div>
				</div>
				<div class="space-y-2.5">
					<div class="px-1 space-y-1.5">
						<div class="h-4 w-36 rounded-full bg-gray-4 animate-pulse" />
						<div class="h-3 w-64 max-w-full rounded-full bg-gray-4 animate-pulse" />
					</div>
					<div class="cap-settings-card overflow-hidden rounded-xl border border-gray-3 bg-gray-2 divide-y divide-gray-3">
						<div class="px-4 py-3.5 space-y-2">
							<div class="h-[15px] w-48 rounded-full bg-gray-4 animate-pulse" />
							<div class="h-3 w-52 max-w-full rounded-full bg-gray-4 animate-pulse" />
						</div>
						<div class="px-4 py-3.5 space-y-2">
							<div class="h-[15px] w-32 rounded-full bg-gray-4 animate-pulse" />
							<div class="h-3 w-72 max-w-full rounded-full bg-gray-4 animate-pulse" />
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}

export default function Settings(props: RouteSectionProps) {
	const navigate = useNavigate();
	const [version, setVersion] = createSignal<string | null>(null);
	const [isCheckingForUpdates, setIsCheckingForUpdates] = createSignal(false);
	const settingsItems = [
		{
			href: "general",
			name: "General",
			icon: IconCapSettings,
		},
		{
			href: "recordings",
			name: "Recordings",
			icon: IconLucideSquarePlay,
		},
		{
			href: "devices",
			name: "Devices",
			icon: IconLucideVideo,
		},
		{
			href: "screenshots",
			name: "Screenshots",
			icon: IconLucideImage,
		},
		{
			href: "hotkeys",
			name: "Shortcuts",
			icon: IconCapHotkeys,
		},
		{
			href: "automations",
			name: "Automations",
			icon: IconLucideZap,
		},
		{
			href: "transcription",
			name: "Transcription",
			icon: IconCapCaptions,
		},
		{
			href: "cli",
			name: "CLI",
			icon: IconLucideTerminal,
		},
		{
			href: "license",
			name: "License",
			icon: IconLucideKeyRound,
		},
		{
			href: "experimental",
			name: "Experimental",
			icon: IconCapSettings,
		},
	];
	onMount(() => {
		void getVersion()
			.then(setVersion)
			.catch((error) => console.error("Failed to load app version:", error));
	});

	const copyVersion = async (appVersion: string) => {
		try {
			await writeText(appVersion);
			toast.success("Version copied to clipboard");
		} catch (error) {
			console.error("Failed to copy app version:", error);
			toast.error("Failed to copy version");
		}
	};

	const checkForUpdates = async () => {
		setIsCheckingForUpdates(true);

		try {
			const update = await commands.updatesCheck();

			if (!update) {
				await dialog.message(
					"You're already using the latest version of Shelf.",
					{
						title: "No Update Available",
						kind: "info",
					},
				);
				return;
			}

			const shouldUpdate = await dialog.confirm(
				`Version ${update.version} of Shelf is available, would you like to install it?`,
				{ title: "Update Shelf", okLabel: "Update", cancelLabel: "Ignore" },
			);

			if (shouldUpdate) navigate("/update");
		} catch (e) {
			console.error("Failed to check for updates:", e);
			await dialog
				.message(
					"Couldn't check for updates. Shelf has no update feed yet, so new versions are built from source \u2014 your data stays where it is.",
					{ title: "Update Shelf", kind: "info" },
				)
				.catch(() => {});
		} finally {
			setIsCheckingForUpdates(false);
		}
	};

	return (
		<div class="cap-settings-shell flex-1 flex flex-row divide-x divide-gray-3 text-[0.875rem] leading-5 overflow-y-hidden">
			<div
				class="cap-settings-sidebar flex flex-col h-full bg-gray-2"
				data-tauri-drag-region
			>
				<div class="cap-settings-window-spacer" data-tauri-drag-region />
				<ul class="cap-settings-nav min-w-48 h-full p-2.5 space-y-1 text-gray-12">
					<For each={settingsItems}>
						{(item) => (
							<li>
								<A
									href={item.href}
									activeClass="bg-gray-5 pointer-events-none"
									class="cap-settings-nav-item rounded-lg h-8 hover:bg-gray-3 text-[13px] px-2 flex flex-row items-center gap-1.5 transition-colors"
								>
									<item.icon class="opacity-60 size-4" aria-hidden="true" />
									<span>{item.name}</span>
								</A>
							</li>
						)}
					</For>
				</ul>
				<div class="cap-settings-account p-2.5 text-left flex flex-col">
					<Show when={version()}>
						{(v) => (
							<div class="mb-2 text-xs text-gray-11 flex flex-col items-start gap-1.5">
								<button
									type="button"
									class="-ml-1 cursor-copy rounded px-1 py-0.5 transition-colors hover:bg-gray-3 hover:text-gray-12"
									title="Copy version to clipboard"
									aria-label={`Copy version ${v()} to clipboard`}
									onClick={() => copyVersion(v())}
								>
									v{v()}
								</button>
								<button
									type="button"
									class="text-gray-11 hover:text-gray-12 underline transition-colors disabled:cursor-default disabled:opacity-50 disabled:hover:text-gray-11"
									disabled={isCheckingForUpdates()}
									onClick={checkForUpdates}
								>
									{isCheckingForUpdates() ? "Checking..." : "Check for updates"}
								</button>
							</div>
						)}
					</Show>
				</div>
			</div>
			<div class="cap-settings-content overflow-y-hidden flex-1 min-w-0">
				<CapErrorBoundary>
					<Suspense fallback={<SettingsContentSkeleton />}>
						{props.children}
					</Suspense>
				</CapErrorBoundary>
			</div>
		</div>
	);
}
