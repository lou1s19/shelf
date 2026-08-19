import { Button } from "@cap/ui-solid";
import Tooltip from "@corvu/tooltip";
import { Collapsible } from "@kobalte/core/collapsible";
import { useNavigate } from "@solidjs/router";
import {
	createQuery,
	queryOptions,
	useQueryClient,
} from "@tanstack/solid-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Menu, MenuItem } from "@tauri-apps/api/menu";
import { ask, confirm } from "@tauri-apps/plugin-dialog";
import { remove } from "@tauri-apps/plugin-fs";
import { type OsType, type } from "@tauri-apps/plugin-os";
import { cx } from "cva";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
	For,
	type JSX,
	onMount,
	type ParentProps,
	Show,
} from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import toast from "solid-toast";
import CapTooltip from "~/components/Tooltip";
import { Input, Slider } from "~/routes/editor/ui";
import { generalSettingsStore, recordingStartSafetyStore } from "~/store";
import { trackEvent } from "~/utils/analytics";
import { createTauriEventListener } from "~/utils/createEventListener";
import {
	deriveGeneralSettings,
	type GeneralSettingsStore,
	RECORDING_START_SAFETY_DEFAULTS,
	type RecordingStartSafetySettings,
} from "~/utils/general-settings";
import { importVideoFromPicker, showImportError } from "~/utils/importMedia";
import { openRecordingFolder } from "~/utils/recording";
import {
	type CaptureWindow,
	commands,
	events,
	type RecordingMetaWithMetadata,
	type StudioRecordingQuality,
	type WindowExclusion,
} from "~/utils/tauri";
import IconLucideAlertTriangle from "~icons/lucide/alert-triangle";
import IconLucideImport from "~icons/lucide/import";
import IconLucidePlus from "~icons/lucide/plus";
import IconLucideSearch from "~icons/lucide/search";
import IconLucideX from "~icons/lucide/x";
import {
	createSettingsSectionScroll,
	Section,
	SectionCard,
	SectionRows,
	SegmentedControl,
	SelectSettingItem,
	SettingItem,
	SettingsPageContent,
	type SettingsView,
	SettingsViewSwitch,
	ToggleSettingItem,
} from "./Setting";

type Recording = {
	meta: RecordingMetaWithMetadata;
	path: string;
	prettyName: string;
	thumbnailPath: string;
};

const Tabs = [
	{
		id: "all",
		label: "Show all",
	},
	{
		id: "instant",
		icon: <IconCapInstant class="invert size-3 dark:invert-0" />,
		label: "Instant",
	},
	{
		id: "studio",
		icon: <IconCapFilmCut class="invert size-3 dark:invert-0" />,
		label: "Studio",
	},
] satisfies { id: string; label: string; icon?: JSX.Element }[];

const PAGE_SIZE = 20;

const hasActiveRecording = (recording: Recording) => {
	const status = recording.meta.status.status;
	return status === "InProgress" || status === "NeedsRemux";
};

const recordingsQuery = queryOptions<Recording[]>({
	queryKey: ["recordings"],
	queryFn: async () => {
		const result = await commands.listRecordings().catch(() => [] as const);

		const recordings = await Promise.all(
			result.map(async (file) => {
				const [path, meta] = file;
				const thumbnailPath = `${path}/screenshots/display.jpg`;

				return {
					meta,
					path,
					prettyName: meta.pretty_name,
					thumbnailPath,
				};
			}),
		);
		return recordings;
	},
	reconcile: "path",
	refetchInterval: (query) => {
		const data = query.state.data;
		if (!data) return false;
		return data.some(hasActiveRecording) ? 2000 : false;
	},
});

export default function Recordings() {
	const [view, setView] = createSignal<SettingsView>("library");
	let scrollContainerRef: HTMLDivElement | undefined;
	const navigate = useNavigate();

	createSettingsSectionScroll({
		page: "recordings",
		container: () => scrollContainerRef,
		navigate: (page) => navigate(`/settings/${page}`),
		onSection: () => setView("settings"),
	});

	return (
		<div
			ref={scrollContainerRef}
			class="cap-settings-page flex relative flex-col w-full h-full custom-scroll"
		>
			<SettingsPageContent class="max-w-none space-y-4">
				<div class="flex px-1">
					<SettingsViewSwitch value={view()} onChange={setView} />
				</div>
				<Show when={view() === "library"} fallback={<RecordingSettings />}>
					<RecordingsLibrary />
				</Show>
			</SettingsPageContent>
		</div>
	);
}

function RecordingsLibrary() {
	const [activeTab, setActiveTab] = createSignal<(typeof Tabs)[number]["id"]>(
		Tabs[0].id,
	);
	const [search, setSearch] = createSignal("");
	const trimmedSearch = createMemo(() => search().trim());
	const normalizedSearch = createMemo(() => trimmedSearch().toLowerCase());
	const [visibleCount, setVisibleCount] = createSignal(PAGE_SIZE);
	const recordings = createQuery(() => recordingsQuery);

	createTauriEventListener(events.recordingDeleted, () => recordings.refetch());

	createEffect(() => {
		activeTab();
		trimmedSearch();
		setVisibleCount(PAGE_SIZE);
	});

	const filteredRecordings = createMemo(() => {
		const data = recordings.data ?? [];
		const scopedRecordings =
			activeTab() === "all"
				? data
				: data.filter((recording) => recording.meta.mode === activeTab());
		const query = normalizedSearch();
		if (!query) return scopedRecordings;
		return scopedRecordings.filter((recording) =>
			recording.prettyName.toLowerCase().includes(query),
		);
	});

	const visibleRecordings = createMemo(() => {
		const items = filteredRecordings();
		if (normalizedSearch()) return items;
		return items.slice(0, visibleCount());
	});

	const hasMoreRecordings = createMemo(
		() => !normalizedSearch() && filteredRecordings().length > visibleCount(),
	);

	const emptyMessage = createMemo(() => {
		const tabLabel =
			activeTab() === "all" ? "recordings" : `${activeTab()} recordings`;
		const prefix = trimmedSearch() ? "No matching" : "No";
		return `${prefix} ${tabLabel}`;
	});

	const handleRecordingClick = (recording: Recording) => {
		trackEvent("recording_view_clicked");
		events.newStudioRecordingAdded.emit({ path: recording.path });
	};

	const handleOpenFolder = (recording: Recording) => {
		trackEvent("recording_folder_clicked");
		openRecordingFolder(recording.path, recording.meta.mode).catch((error) => {
			console.error("Failed to open recording folder:", error);
		});
	};

	const handleCopyVideoToClipboard = (path: string) => {
		trackEvent("recording_copy_clicked");
		commands.copyVideoToClipboard(path);
	};

	const handleOpenEditor = (path: string) => {
		trackEvent("recording_editor_clicked");
		commands.showWindow({
			Editor: { project_path: path },
		});
	};

	const handleVideoImport = async () => {
		try {
			await importVideoFromPicker();
		} catch (e) {
			console.error("Failed to import video:", e);
			await showImportError("video", e);
		}
	};

	return (
		<Section
			title="Recordings"
			description="Manage your recordings and perform actions."
			right={
				<Button
					variant="gray"
					size="sm"
					class="h-[36px] px-3 shrink-0 flex items-center gap-1.5"
					onClick={handleVideoImport}
				>
					<IconLucideImport class="size-3.5" />
					<span>Import</span>
				</Button>
			}
		>
			<Show
				when={recordings.data && recordings.data.length > 0}
				fallback={
					<p class="text-center text-(--text-tertiary) absolute flex items-center justify-center w-full h-full">
						No recordings found
					</p>
				}
			>
				<div class="flex flex-col gap-3 pb-4 w-full border-b border-gray-2">
					<div class="flex flex-wrap gap-3 items-center">
						<For each={Tabs}>
							{(tab) => (
								<div
									class={cx(
										"flex gap-1.5 items-center transition-colors duration-200 p-2 px-3 border rounded-full",
										activeTab() === tab.id
											? "bg-gray-5 cursor-default border-gray-5"
											: "bg-transparent hover:bg-gray-3 border-gray-5",
									)}
									onClick={() => setActiveTab(tab.id)}
								>
									{tab.icon && tab.icon}
									<p class="text-xs text-gray-12">{tab.label}</p>
								</div>
							)}
						</For>
					</div>
					<div class="relative w-full max-w-[260px] h-[36px] flex items-center">
						<IconLucideSearch class="absolute left-2 top-1/2 -translate-y-1/2 pointer-events-none size-3 text-gray-10" />
						<Input
							type="search"
							class="py-2 pl-6 h-full w-full"
							value={search()}
							onInput={(event) => setSearch(event.currentTarget.value)}
							onKeyDown={(event) => {
								if (event.key === "Escape" && search()) {
									event.preventDefault();
									setSearch("");
								}
							}}
							placeholder="Search"
							autoCapitalize="off"
							autocorrect="off"
							autocomplete="off"
							spellcheck={false}
							aria-label="Search recordings"
						/>
					</div>
				</div>

				<div class="flex relative flex-col flex-1 mt-4 rounded-xl border custom-scroll bg-gray-2 border-gray-3">
					<Show when={filteredRecordings().length === 0}>
						<p class="text-center text-(--text-tertiary) absolute flex items-center justify-center w-full h-full">
							{emptyMessage()}
						</p>
					</Show>
					<ul class="flex flex-col w-full text-(--text-primary)">
						<For each={visibleRecordings()}>
							{(recording) => (
								<RecordingItem
									recording={recording}
									onClick={() => handleRecordingClick(recording)}
									onOpenFolder={() => handleOpenFolder(recording)}
									onOpenEditor={() => handleOpenEditor(recording.path)}
									onCopyVideoToClipboard={() =>
										handleCopyVideoToClipboard(recording.path)
									}
								/>
							)}
						</For>
					</ul>
					<Show when={hasMoreRecordings()}>
						<div class="flex justify-center p-3 border-t border-gray-3">
							<Button
								variant="gray"
								size="sm"
								onClick={() =>
									setVisibleCount((count) =>
										Math.min(count + PAGE_SIZE, filteredRecordings().length),
									)
								}
							>
								Load more
							</Button>
						</div>
					</Show>
				</div>
			</Show>
		</Section>
	);
}

function RecordingItem(props: {
	recording: Recording;
	onClick: () => void;
	onOpenFolder: () => void;
	onOpenEditor: () => void;
	onCopyVideoToClipboard: () => void;
}) {
	const [imageExists, setImageExists] = createSignal(true);
	const mode = () => props.recording.meta.mode;
	const firstLetterUpperCase = () =>
		mode().charAt(0).toUpperCase() + mode().slice(1);

	const queryClient = useQueryClient();
	const studioCompleteCheck = () =>
		mode() === "studio" && props.recording.meta.status.status === "Complete";

	return (
		<li
			onClick={() => {
				if (studioCompleteCheck()) {
					props.onOpenEditor();
				}
			}}
			class={cx(
				"flex flex-row justify-between p-3 not-last:border-b not-last:border-gray-3 items-center w-full  transition-colors duration-200",
				studioCompleteCheck() ? "hover:bg-gray-3" : "cursor-default",
			)}
		>
			<div class="flex gap-5 items-center">
				<Show
					when={imageExists()}
					fallback={<div class="mr-4 rounded-sm bg-gray-10 size-11" />}
				>
					<img
						class="object-cover rounded-sm size-12"
						alt="Recording thumbnail"
						src={`${convertFileSrc(
							props.recording.thumbnailPath,
						)}?t=${Date.now()}`}
						onError={() => setImageExists(false)}
					/>
				</Show>
				<div class="flex flex-col gap-2">
					<span>{props.recording.prettyName}</span>
					<div class="flex space-x-1">
						<div
							class={cx(
								"px-2 py-0.5 flex items-center gap-1.5 font-medium text-[11px] text-gray-12 rounded-full w-fit",
								mode() === "instant" ? "bg-blue-100" : "bg-gray-4",
							)}
						>
							{mode() === "instant" ? (
								<IconCapInstant class="invert size-2.5 dark:invert-0" />
							) : (
								<IconCapFilmCut class="invert size-2.5 dark:invert-0" />
							)}
							<p>{firstLetterUpperCase()}</p>
						</div>

						<Show when={props.recording.meta.clip_count > 1}>
							<div class="px-2 py-0.5 flex items-center font-medium text-[11px] text-gray-12 rounded-full w-fit bg-gray-4">
								<p>{props.recording.meta.clip_count} clips</p>
							</div>
						</Show>

						<Show when={props.recording.meta.status.status === "InProgress"}>
							<div
								class={cx(
									"px-2 py-0.5 flex items-center gap-1.5 font-medium text-[11px] text-gray-12 rounded-full w-fit bg-blue-500 leading-none text-center",
								)}
							>
								<IconPhRecordFill class="invert size-2.5 dark:invert-0" />
								<p>Recording in progress</p>
							</div>
						</Show>

						<Show when={props.recording.meta.status.status === "Failed"}>
							<CapTooltip
								content={
									<span>
										{props.recording.meta.status.status === "Failed"
											? props.recording.meta.status.error
											: ""}
									</span>
								}
							>
								<div
									class={cx(
										"px-2 py-0.5 flex items-center gap-1.5 font-medium text-[11px] text-gray-12 rounded-full w-fit bg-red-9 leading-none text-center",
									)}
								>
									<IconPhWarningBold class="invert size-2.5 dark:invert-0" />
									<p>Recording failed</p>
								</div>
							</CapTooltip>
						</Show>
					</div>
				</div>
			</div>
			<div class="flex gap-2 items-center">
				<Show when={mode() === "studio"}>
					<TooltipIconButton
						tooltipText="Edit"
						onClick={async () => {
							if (
								props.recording.meta.status.status === "Failed" &&
								!(await confirm(
									"The recording failed so this file may have issues in the editor! If your having issues recovering the file please reach out to support!",
									{
										title: "Recording is potentially corrupted",
										kind: "warning",
									},
								))
							)
								return;
							props.onOpenEditor();
						}}
						disabled={props.recording.meta.status.status === "InProgress"}
					>
						<IconLucideEdit class="size-4" />
					</TooltipIconButton>
				</Show>
				<TooltipIconButton
					tooltipText="Open recording bundle"
					onClick={() => {
						props.onOpenFolder();
					}}
				>
					<IconLucideFolder class="size-4" />
				</TooltipIconButton>
				<TooltipIconButton
					tooltipText="Delete"
					onClick={async () => {
						if (!(await ask("Are you sure you want to delete this recording?")))
							return;
						await remove(props.recording.path, { recursive: true });

						queryClient.refetchQueries(recordingsQuery);
					}}
				>
					<IconCapTrash class="size-4" />
				</TooltipIconButton>
			</div>
		</li>
	);
}

function TooltipIconButton(
	props: ParentProps<{
		onClick: () => void;
		tooltipText: string;
		disabled?: boolean;
	}>,
) {
	return (
		<Tooltip>
			<Tooltip.Trigger
				onClick={(e: MouseEvent) => {
					e.stopPropagation();
					props.onClick();
				}}
				disabled={props.disabled}
				class="p-2.5 opacity-70 will-change-transform hover:opacity-100 rounded-full transition-all duration-200 hover:bg-gray-3 dark:hover:bg-gray-5 disabled:pointer-events-none disabled:opacity-45 disabled:hover:opacity-45"
			>
				{props.children}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Content class="py-2 px-3 font-medium bg-gray-2 text-gray-12 border border-gray-3 text-xs rounded-lg animate-in fade-in slide-in-from-top-0.5">
					{props.tooltipText}
				</Tooltip.Content>
			</Tooltip.Portal>
		</Tooltip>
	);
}

const MAX_FPS_OPTIONS = [
	{ value: 24, label: "24 FPS" },
	{ value: 25, label: "25 FPS" },
	{ value: 30, label: "30 FPS" },
	{ value: 60, label: "60 FPS (Recommended)" },
	{ value: 120, label: "120 FPS" },
] satisfies {
	value: number;
	label: string;
}[];

const DEFAULT_PROJECT_NAME_TEMPLATE =
	"{target_name} ({target_kind}) {date} {time}";
const DEFAULT_INSTANT_MODE_MAX_RESOLUTION = 1920;

const getExclusionPrimaryLabel = (entry: WindowExclusion) =>
	entry.ownerName ?? entry.windowTitle ?? entry.bundleIdentifier ?? "Unknown";

const getExclusionSecondaryLabel = (entry: WindowExclusion) => {
	if (entry.ownerName && entry.windowTitle) {
		return entry.windowTitle;
	}

	if (entry.bundleIdentifier && (entry.ownerName || entry.windowTitle)) {
		return entry.bundleIdentifier;
	}

	return entry.bundleIdentifier ?? null;
};

const getWindowOptionLabel = (window: CaptureWindow) => {
	const parts = [window.owner_name];
	if (window.name && window.name !== window.owner_name) {
		parts.push(window.name);
	}
	return parts.join(" • ");
};

const isSameExclusion = (a: WindowExclusion, b: WindowExclusion) =>
	(a.bundleIdentifier ?? null) === (b.bundleIdentifier ?? null) &&
	(a.ownerName ?? null) === (b.ownerName ?? null) &&
	(a.windowTitle ?? null) === (b.windowTitle ?? null);

const coversDefaultExclusion = (
	entry: WindowExclusion,
	defaultEntry: WindowExclusion,
) => {
	if (isSameExclusion(entry, defaultEntry)) return true;
	if (
		defaultEntry.windowTitle &&
		entry.windowTitle === defaultEntry.windowTitle
	) {
		return true;
	}
	if (
		defaultEntry.bundleIdentifier &&
		entry.bundleIdentifier === defaultEntry.bundleIdentifier
	) {
		return true;
	}
	if (defaultEntry.ownerName && entry.ownerName === defaultEntry.ownerName) {
		return !entry.windowTitle || entry.windowTitle === defaultEntry.windowTitle;
	}
	return false;
};

function RecordingSettings() {
	const [stores] = createResource(() =>
		Promise.all([generalSettingsStore.get(), recordingStartSafetyStore.get()]),
	);

	return (
		<Show when={stores()} keyed>
			{(stores) => (
				<RecordingSettingsInner
					initialStore={stores[0] ?? null}
					initialRecordingStartSafety={
						stores[1] ?? RECORDING_START_SAFETY_DEFAULTS
					}
				/>
			)}
		</Show>
	);
}

function RecordingSettingsInner(props: {
	initialStore: GeneralSettingsStore | null;
	initialRecordingStartSafety: RecordingStartSafetySettings;
}) {
	const [settings, setSettings] = createStore<GeneralSettingsStore>(
		deriveGeneralSettings(props.initialStore),
	);
	const [
		confirmBeforeRecordingWithoutMicrophone,
		setConfirmBeforeRecordingWithoutMicrophone,
	] = createSignal(
		props.initialRecordingStartSafety.confirmBeforeRecordingWithoutMicrophone,
	);
	const instantModeMaxResolution = createMemo(
		() =>
			settings.instantModeMaxResolution ?? DEFAULT_INSTANT_MODE_MAX_RESOLUTION,
	);

	createEffect(() => {
		setSettings(reconcile(deriveGeneralSettings(props.initialStore)));
	});

	const [windows, { refetch: refetchWindows }] = createResource(
		async () => {
			// Fetch windows with a small delay to avoid blocking initial render
			await new Promise((resolve) => setTimeout(resolve, 100));
			return commands.listCaptureWindows();
		},
		{
			initialValue: [] as CaptureWindow[],
		},
	);
	const [defaultExcludedWindows] = createResource(
		() => commands.getDefaultExcludedWindows(),
		{
			initialValue: [] as WindowExclusion[],
		},
	);

	const handleChange = async <K extends keyof typeof settings>(
		key: K,
		value: (typeof settings)[K],
	) => {
		const previousValue = settings[key];
		setSettings(key as keyof GeneralSettingsStore, value);
		try {
			await generalSettingsStore.set({ [key]: value });
		} catch (error) {
			setSettings(key as keyof GeneralSettingsStore, previousValue);
			console.error(`Failed to update ${key}`, error);
		}
	};

	const handleRecordingStartSafetyChange = async (value: boolean) => {
		const previousValue = confirmBeforeRecordingWithoutMicrophone();
		setConfirmBeforeRecordingWithoutMicrophone(value);
		try {
			await recordingStartSafetyStore.set({
				confirmBeforeRecordingWithoutMicrophone: value,
			});
		} catch (error) {
			setConfirmBeforeRecordingWithoutMicrophone(previousValue);
			console.error("Failed to update recording start safety", error);
		}
	};

	const ostype: OsType = type();
	const excludedWindows = createMemo(() => settings.excludedWindows ?? []);
	const missingDefaultExclusions = createMemo(() =>
		defaultExcludedWindows().filter(
			(defaultEntry) =>
				!excludedWindows().some((entry) =>
					coversDefaultExclusion(entry, defaultEntry),
				),
		),
	);

	const matchesExclusion = (
		exclusion: WindowExclusion,
		window: CaptureWindow,
	) => {
		const bundleMatch = exclusion.bundleIdentifier
			? window.bundle_identifier === exclusion.bundleIdentifier
			: false;
		if (bundleMatch) return true;

		const ownerMatch = exclusion.ownerName
			? window.owner_name === exclusion.ownerName
			: false;

		if (exclusion.ownerName && exclusion.windowTitle) {
			return ownerMatch && window.name === exclusion.windowTitle;
		}

		if (ownerMatch && exclusion.ownerName) {
			return true;
		}

		if (exclusion.windowTitle) {
			return window.name === exclusion.windowTitle;
		}

		return false;
	};

	const isManagedWindowsApp = (window: CaptureWindow) => {
		const bundle = window.bundle_identifier?.toLowerCase() ?? "";
		if (bundle.includes("so.cap.desktop")) {
			return true;
		}
		return window.owner_name.toLowerCase().includes("cap");
	};

	const isWindowAvailable = (window: CaptureWindow) => {
		if (excludedWindows().some((entry) => matchesExclusion(entry, window))) {
			return false;
		}
		if (ostype === "windows") {
			return isManagedWindowsApp(window);
		}
		return true;
	};

	const availableWindows = createMemo(() => {
		const data = windows() ?? [];
		return data.filter(isWindowAvailable);
	});

	const refreshAvailableWindows = async (): Promise<CaptureWindow[]> => {
		try {
			const refreshed = (await refetchWindows()) ?? windows() ?? [];
			return refreshed.filter(isWindowAvailable);
		} catch (error) {
			console.error("Failed to refresh available windows", error);
			return availableWindows();
		}
	};

	const applyExcludedWindows = async (windows: WindowExclusion[]) => {
		setSettings("excludedWindows", windows);
		try {
			await generalSettingsStore.set({ excludedWindows: windows });
			await commands.refreshWindowContentProtection();
			if (ostype === "macos") {
				await events.requestScreenCapturePrewarm.emit({ force: true });
			}
		} catch (error) {
			console.error("Failed to update excluded windows", error);
		}
	};

	const handleRemoveExclusion = async (index: number) => {
		const current = [...excludedWindows()];
		current.splice(index, 1);
		await applyExcludedWindows(current);
	};

	const handleAddWindow = async (window: CaptureWindow) => {
		const windowTitle = window.bundle_identifier ? null : window.name;

		const next = [
			...excludedWindows(),
			{
				bundleIdentifier: window.bundle_identifier ?? null,
				ownerName: window.owner_name ?? null,
				windowTitle,
			},
		];
		await applyExcludedWindows(next);
	};

	const handleResetExclusions = async () => {
		const defaults = await commands.getDefaultExcludedWindows();
		await applyExcludedWindows(defaults);
	};

	return (
		<div class="space-y-7 max-w-[42rem]">
			<Section
				title="Quality"
				description="Encoder profiles for Instant and Studio recordings."
			>
				<SectionRows>
					<InstantQualitySetting
						value={instantModeMaxResolution()}
						onChange={(value) =>
							handleChange("instantModeMaxResolution", value)
						}
					/>
					<StudioQualitySubsection
						value={settings.studioRecordingQuality ?? "balanced"}
						onChange={(value) => handleChange("studioRecordingQuality", value)}
					/>
				</SectionRows>
			</Section>

			<Section
				title="Recording"
				description="Behaviour while you record and after you stop."
			>
				<SectionRows>
					<SelectSettingItem
						label="Countdown"
						description="Wait before the recording starts."
						value={settings.recordingCountdown ?? 0}
						onChange={(value) => handleChange("recordingCountdown", value)}
						options={[
							{ text: "Off", value: 0 },
							{ text: "3 seconds", value: 3 },
							{ text: "5 seconds", value: 5 },
							{ text: "10 seconds", value: 10 },
						]}
					/>
					<ToggleSettingItem
						label="Confirm before recording without a microphone"
						description="Require confirmation when no microphone is selected or the selected microphone is unavailable."
						value={confirmBeforeRecordingWithoutMicrophone()}
						onChange={handleRecordingStartSafetyChange}
					/>
					<SelectSettingItem
						label="After a Studio recording"
						description="What happens once you stop a Studio recording."
						value={settings.postStudioRecordingBehaviour ?? "openEditor"}
						onChange={(value) =>
							handleChange("postStudioRecordingBehaviour", value)
						}
						options={[
							{ text: "Open editor", value: "openEditor" },
							{ text: "Show in overlay", value: "showOverlay" },
						]}
					/>
					<ToggleSettingItem
						label="Crash-recoverable recording"
						description="Record in fragments that can be recovered after a crash or power loss. Slightly larger files during capture."
						value={settings.crashRecoveryRecording ?? true}
						onChange={(value) => handleChange("crashRecoveryRecording", value)}
					/>
				</SectionRows>
			</Section>

			<Section
				title="Capture"
				description="What ends up in the recorded picture."
			>
				<SectionRows>
					<SelectSettingItem
						label="Max capture framerate"
						description={
							(settings.maxFps ?? 60) > 60
								? "Maximum framerate for screen capture. Higher values may cause drops or increased CPU usage on some systems."
								: "Maximum framerate for screen capture."
						}
						value={settings.maxFps ?? 60}
						onChange={(value) => handleChange("maxFps", value)}
						options={MAX_FPS_OPTIONS.map((option) => ({
							text: option.label,
							value: option.value,
						}))}
					/>
					<ToggleSettingItem
						label="Custom cursor capture (Studio)"
						description="Capture cursor state separately so you can adjust size and smoothing in the editor."
						value={!!settings.custom_cursor_capture2}
						onChange={(value) => handleChange("custom_cursor_capture2", value)}
					/>
					<ToggleSettingItem
						label="Auto zoom on clicks"
						description="Automatically add zoom segments around mouse clicks in Studio recordings."
						value={!!settings.autoZoomOnClicks}
						onChange={(value) => handleChange("autoZoomOnClicks", value)}
					/>
					<SettingItem
						label="Default zoom amount"
						description="Zoom level for newly created and auto-generated zoom segments."
					>
						<div class="flex gap-2 items-center w-52">
							<Slider
								class="flex-1"
								value={[settings.defaultZoomAmount ?? 1.5]}
								onChange={(v) => setSettings("defaultZoomAmount", v[0])}
								onChangeEnd={(v) => handleChange("defaultZoomAmount", v[0])}
								minValue={1}
								maxValue={4.5}
								step={0.1}
								formatTooltip="x"
							/>
							<span class="w-9 text-xs text-right text-gray-11 tabular-nums">
								{`${(settings.defaultZoomAmount ?? 1.5).toFixed(1)}x`}
							</span>
						</div>
					</SettingItem>
					<ToggleSettingItem
						label="Capture keyboard presses"
						description="Record key presses so you can add keyboard overlays in the editor."
						value={!!settings.captureKeyboardEvents}
						onChange={(value) => handleChange("captureKeyboardEvents", value)}
					/>
					<ToggleSettingItem
						label="Draw the MacBook notch on screen recordings"
						description="Automatically restores the notch for new screen and area recordings when the selected region contains the complete notch. External displays, partial areas, and window recordings are left alone. Each recording can override it in the editor."
						value={!!settings.macbookNotchOverlay}
						onChange={(value) => handleChange("macbookNotchOverlay", value)}
					/>
				</SectionRows>
			</Section>

			<StorageSection
				recordingsPath={settings.recordingsPath ?? null}
				onPick={async () => {
					try {
						const path = await commands.pickRecordingsFolder();
						if (path !== null) {
							setSettings("recordingsPath", path);
							await offerRecordingsMigration();
						}
					} catch (e) {
						toast.error(
							`Failed to choose recordings folder: ${e instanceof Error ? e.message : String(e)}`,
						);
					}
				}}
				onReset={async () => {
					try {
						await commands.resetRecordingsFolder();
						setSettings("recordingsPath", null);
						await offerRecordingsMigration();
					} catch (e) {
						toast.error(
							`Failed to reset recordings folder: ${e instanceof Error ? e.message : String(e)}`,
						);
					}
				}}
			/>

			<DefaultProjectNameCard
				onChange={(value) => handleChange("defaultProjectNameTemplate", value)}
				value={settings.defaultProjectNameTemplate ?? null}
			/>

			<ExcludedWindowsCard
				excludedWindows={excludedWindows()}
				missingDefaultExclusions={missingDefaultExclusions()}
				availableWindows={availableWindows()}
				onRequestAvailableWindows={refreshAvailableWindows}
				onRemove={handleRemoveExclusion}
				onAdd={handleAddWindow}
				onReset={handleResetExclusions}
				isLoading={windows.loading}
				isWindows={ostype === "windows"}
			/>
		</div>
	);
}

async function offerRecordingsMigration() {
	let count = 0;
	try {
		count = await commands.countRecordingsToMigrate();
	} catch {
		// Recordings in other folders stay visible in the library either way,
		// so a failed scan just means we don't offer the move.
		return;
	}
	if (count === 0) return;

	const plural = count === 1 ? "recording" : "recordings";
	const shouldMove = await confirm(
		`Move your ${count} existing ${plural} to the new location? Recordings stay in your library either way.`,
	);
	if (!shouldMove) return;

	const toastId = toast.loading(`Moving ${count} ${plural}…`);
	let unlisten: (() => void) | undefined;
	try {
		unlisten = await events.recordingsMigrationProgress.listen((e) => {
			toast.loading(
				`Moving recordings… ${Math.min(e.payload.done + 1, e.payload.total)}/${e.payload.total}`,
				{ id: toastId },
			);
		});

		const summary = await commands.migrateRecordingsToCurrentDir();

		const parts = [
			`Moved ${summary.moved} ${summary.moved === 1 ? "recording" : "recordings"}`,
		];
		if (summary.skippedInUse > 0) {
			parts.push(`${summary.skippedInUse} in use, left in place`);
		}
		if (summary.failed.length > 0) {
			parts.push(
				`${summary.failed.length} failed, kept in the original folder`,
			);
			toast.error(parts.join(" · "), { id: toastId });
		} else {
			toast.success(parts.join(" · "), { id: toastId });
		}
	} catch (e) {
		toast.error(
			`Failed to move recordings: ${e instanceof Error ? e.message : String(e)}`,
			{ id: toastId },
		);
	} finally {
		unlisten?.();
	}
}

function StorageSection(props: {
	recordingsPath: string | null;
	onPick: () => Promise<void>;
	onReset: () => Promise<void>;
}) {
	const defaultLabel = "Default (Application Support)";
	const displayPath = () => props.recordingsPath ?? defaultLabel;
	const isCustom = () => props.recordingsPath !== null;

	return (
		<Section title="Storage" description="Where Shelf saves your recordings.">
			<SectionCard padded>
				<div class="flex flex-col gap-3">
					<div class="flex items-center gap-2 px-3 py-2 rounded-lg bg-gray-3 border border-gray-4 min-w-0">
						<span class="flex-1 text-xs text-gray-12 truncate font-mono">
							{displayPath()}
						</span>
					</div>
					<div class="flex justify-end gap-2">
						<Show when={isCustom()}>
							<Button size="sm" variant="gray" onClick={props.onReset}>
								Reset to Default
							</Button>
						</Show>
						<Button size="sm" variant="dark" onClick={props.onPick}>
							Choose Folder
						</Button>
					</div>
				</div>
			</SectionCard>
		</Section>
	);
}

type StudioQualityTier = {
	value: StudioRecordingQuality;
	label: string;
	summary: string;
	bestFor: string;
};

const STUDIO_QUALITY_TIERS: StudioQualityTier[] = [
	{
		value: "compatibility",
		label: "Compatibility",
		summary: "Lower bitrate to keep older or low-power machines smooth.",
		bestFor: "Older Intel Macs, 8GB MacBook Air, weaker laptops.",
	},
	{
		value: "balanced",
		label: "Balanced",
		summary: "Sharp footage with sensible CPU and disk usage.",
		bestFor: "Most modern Macs and PCs with 16GB+ RAM.",
	},
	{
		value: "ultra",
		label: "Ultra",
		summary: "Maximum detail for color-graded, large-display edits.",
		bestFor: "M-series Pro/Max, discrete GPUs, 32GB+ RAM, NVMe.",
	},
];

type InstantResolutionTier = {
	value: number;
	label: string;
	summary: string;
};

const INSTANT_RESOLUTION_TIERS: InstantResolutionTier[] = [
	{ value: 1280, label: "720p", summary: "Smallest files, fastest to write." },
	{
		value: 1920,
		label: "1080p",
		summary: "Recommended. Sharp on most displays.",
	},
	{ value: 2560, label: "1440p", summary: "More detail for desktop content." },
	{ value: 3840, label: "4K", summary: "Max clarity. Largest files." },
];

function StudioQualitySubsection(props: {
	value: StudioRecordingQuality;
	onChange: (value: StudioRecordingQuality) => void;
}) {
	const currentTier = createMemo(
		() =>
			STUDIO_QUALITY_TIERS.find((t) => t.value === props.value) ??
			STUDIO_QUALITY_TIERS[1],
	);

	return (
		<div
			id="settings-section-studio-quality"
			class="flex flex-col gap-3 px-4 py-4"
		>
			<div class="flex justify-between items-start gap-4">
				<div class="flex flex-col gap-0.5 min-w-0">
					<p class="text-[13px] text-gray-12">Studio mode</p>
					<p class="text-xs leading-snug text-gray-10">
						Encoder profile for local Studio recordings.
					</p>
				</div>
				<SegmentedControl
					value={props.value}
					onChange={props.onChange}
					options={STUDIO_QUALITY_TIERS.map((tier) => ({
						value: tier.value,
						label: tier.label,
					}))}
				/>
			</div>
			<div class="flex flex-col gap-1.5 px-3 py-2.5 rounded-lg bg-gray-3">
				<p class="text-xs text-gray-12">{currentTier().summary}</p>
				<p class="text-[11px] text-gray-10 leading-snug">
					<span class="text-gray-11">Best for:</span> {currentTier().bestFor}
				</p>
			</div>
		</div>
	);
}

function InstantQualitySetting(props: {
	value: number;
	onChange: (value: number) => void;
}) {
	const currentTier = createMemo(
		() =>
			INSTANT_RESOLUTION_TIERS.find((t) => t.value === props.value) ??
			INSTANT_RESOLUTION_TIERS[0],
	);

	return (
		<SettingItem
			id="settings-section-instant-quality"
			label="Instant Mode quality"
			description="Choose the maximum resolution for Instant recordings."
		>
			<div class="flex flex-col items-end gap-1.5">
				<div class="inline-flex p-0.5 rounded-lg border border-gray-3 bg-gray-3">
					<For each={INSTANT_RESOLUTION_TIERS}>
						{(tier) => {
							const isSelected = () => props.value === tier.value;
							return (
								<button
									type="button"
									onClick={() => props.onChange(tier.value)}
									class={cx(
										"px-3 py-1 text-xs font-medium rounded-md transition-[background-color,color,box-shadow]",
										isSelected()
											? "bg-gray-1 text-gray-12 shadow-sm"
											: "text-gray-10 hover:text-gray-12",
									)}
								>
									{tier.label}
								</button>
							);
						}}
					</For>
				</div>
				<p class="text-[11px] leading-snug text-right text-gray-10">
					{currentTier().summary}
				</p>
			</div>
		</SettingItem>
	);
}

function DefaultProjectNameCard(props: {
	value: string | null;
	onChange: (name: string | null) => Promise<void>;
}) {
	const MOMENT_EXAMPLE_TEMPLATE = "{moment:DDDD, MMMM D, YYYY h:mm A}";
	const macos = type() === "macos";
	const today = new Date();
	const datetime = new Date(
		today.getFullYear(),
		today.getMonth(),
		today.getDate(),
		macos ? 9 : 12,
		macos ? 41 : 0,
		0,
		0,
	).toISOString();

	let inputRef: HTMLInputElement | undefined;

	const dateString = today.toISOString().split("T")[0];
	const initialTemplate = () => props.value ?? DEFAULT_PROJECT_NAME_TEMPLATE;

	const [inputValue, setInputValue] = createSignal<string>(initialTemplate());
	const [preview, setPreview] = createSignal<string | null>(null);
	const [momentExample, setMomentExample] = createSignal("");

	async function updatePreview(val = inputValue()) {
		const formatted = await commands.formatProjectName(
			val,
			macos ? "Safari" : "Chrome",
			"Window",
			"instant",
			datetime,
		);
		setPreview(formatted);
	}

	onMount(() => {
		commands
			.formatProjectName(
				MOMENT_EXAMPLE_TEMPLATE,
				macos ? "Safari" : "Chrome",
				"Window",
				"instant",
				datetime,
			)
			.then(setMomentExample);

		const seed = initialTemplate();
		setInputValue(seed);
		if (inputRef) inputRef.value = seed;
		updatePreview(seed);
	});

	const isSaveDisabled = () => {
		const input = inputValue();
		return (
			!input ||
			input === (props.value ?? DEFAULT_PROJECT_NAME_TEMPLATE) ||
			input.length <= 3
		);
	};

	function CodeView(props: { children: string }) {
		return (
			<button
				type="button"
				title="Click to copy"
				class="px-1.5 py-0.5 mx-0.5 font-mono text-[11px] rounded-md transition-[background-color,color,transform] duration-150 ease-out bg-gray-3 hover:bg-gray-4 active:scale-95 text-gray-12"
				onClick={() => commands.writeClipboardString(props.children)}
			>
				{props.children}
			</button>
		);
	}

	return (
		<Section
			title="Default project name"
			description="Template used for new recordings, screenshots and exported files."
			right={
				<>
					<Button
						size="sm"
						variant="gray"
						disabled={
							inputValue() === DEFAULT_PROJECT_NAME_TEMPLATE &&
							inputValue() !== props.value
						}
						onClick={async () => {
							await props.onChange(null);
							const newTemplate = initialTemplate();
							setInputValue(newTemplate);
							if (inputRef) inputRef.value = newTemplate;
							await updatePreview(newTemplate);
						}}
					>
						Reset
					</Button>
					<Button
						size="sm"
						variant="dark"
						disabled={isSaveDisabled()}
						onClick={async () => {
							await props.onChange(inputValue() ?? null);
							await updatePreview();
						}}
					>
						Save
					</Button>
				</>
			}
		>
			<SectionCard padded>
				<div class="flex flex-col gap-3">
					<Input
						autocorrect="off"
						ref={inputRef}
						type="text"
						class="bg-gray-3 font-mono"
						value={inputValue()}
						onInput={(e) => {
							setInputValue(e.currentTarget.value);
							updatePreview(e.currentTarget.value);
						}}
					/>

					<div class="flex gap-2 items-center px-3 py-2 rounded-lg border border-dashed bg-gray-3 border-gray-5">
						<IconCapLogo class="pointer-events-none size-4 shrink-0" />
						<p class="text-xs text-gray-12 whitespace-pre-wrap">{preview()}</p>
					</div>

					<Collapsible class="w-full rounded-lg">
						<Collapsible.Trigger class="inline-flex gap-1 items-center text-xs transition-colors text-gray-10 hover:text-gray-12 group">
							<IconCapChevronDown class="size-3.5 data-group-expanded:rotate-180 transition-transform duration-200" />
							<span>Available placeholders</span>
						</Collapsible.Trigger>

						<Collapsible.Content class="space-y-3 pt-3 text-xs text-gray-12 opacity-0 transition animate-collapsible-up data-expanded:animate-collapsible-down data-expanded:opacity-100">
							<p class="text-gray-10">
								Click any placeholder to copy it. Time supports custom formats
								via <code class="text-gray-12">{"{moment:HH:mm}"}</code>.
							</p>

							<div class="space-y-1">
								<p class="font-medium text-gray-12">Recording mode</p>
								<p>
									<CodeView>{"{recording_mode}"}</CodeView> → "Studio",
									"Instant", or "Screenshot"
								</p>
								<p>
									<CodeView>{"{mode}"}</CodeView> → "studio", "instant", or
									"screenshot"
								</p>
							</div>

							<div class="space-y-1">
								<p class="font-medium text-gray-12">Target</p>
								<p>
									<CodeView>{"{target_kind}"}</CodeView> → "Display", "Window",
									or "Area"
								</p>
								<p>
									<CodeView>{"{target_name}"}</CodeView> → Monitor name or
									window title.
								</p>
							</div>

							<div class="space-y-1">
								<p class="font-medium text-gray-12">Date &amp; time</p>
								<p>
									<CodeView>{"{date}"}</CodeView> → {dateString}
								</p>
								<p>
									<CodeView>{"{time}"}</CodeView> →{" "}
									{macos ? "09:41 AM" : "12:00 PM"}
								</p>
								<p class="flex flex-col items-start pt-1">
									<CodeView>{MOMENT_EXAMPLE_TEMPLATE}</CodeView> →{" "}
									{momentExample()}
								</p>
							</div>
						</Collapsible.Content>
					</Collapsible>
				</div>
			</SectionCard>
		</Section>
	);
}

function ExcludedWindowsCard(props: {
	excludedWindows: WindowExclusion[];
	missingDefaultExclusions: WindowExclusion[];
	availableWindows: CaptureWindow[];
	onRequestAvailableWindows: () => Promise<CaptureWindow[]>;
	onRemove: (index: number) => Promise<void>;
	onAdd: (window: CaptureWindow) => Promise<void>;
	onReset: () => Promise<void>;
	isLoading: boolean;
	isWindows: boolean;
}) {
	const hasExclusions = () => props.excludedWindows.length > 0;
	const hasMissingDefaultExclusions = () =>
		props.missingDefaultExclusions.length > 0;
	const missingDefaultLabels = () =>
		props.missingDefaultExclusions.map(getExclusionPrimaryLabel).join(", ");
	const canAdd = () => !props.isLoading;
	const handleResetClick = () => {
		if (props.isLoading) return;
		void props.onReset();
	};

	const handleAddClick = async (event: MouseEvent) => {
		event.preventDefault();
		event.stopPropagation();

		if (!canAdd()) return;

		// Use available windows if we have them, otherwise fetch
		let windows = props.availableWindows;

		// Only refresh if we don't have any windows cached
		if (!windows.length) {
			try {
				windows = await props.onRequestAvailableWindows();
			} catch (error) {
				console.error("Failed to fetch windows:", error);
				return;
			}
		}

		if (!windows.length) {
			console.log("No available windows to exclude");
			return;
		}

		try {
			const items = await Promise.all(
				windows.map((window) =>
					MenuItem.new({
						text: getWindowOptionLabel(window),
						action: () => {
							void props.onAdd(window);
						},
					}),
				),
			);

			const menu = await Menu.new({ items });

			// Save scroll position before popup
			const scrollPos = window.scrollY;

			await menu.popup();
			await menu.close();

			// Restore scroll position after menu closes
			requestAnimationFrame(() => {
				window.scrollTo(0, scrollPos);
			});
		} catch (error) {
			console.error("Error showing window menu:", error);
		}
	};

	return (
		<Section
			title="Excluded windows"
			description={
				props.isWindows
					? "Hide windows from recordings. On Windows, only Shelf windows can be excluded."
					: "Hide windows from recordings."
			}
			right={
				<>
					<Button
						variant="gray"
						size="sm"
						disabled={props.isLoading}
						onClick={handleResetClick}
					>
						Reset
					</Button>
					<Button
						variant="dark"
						size="sm"
						disabled={!canAdd()}
						onClick={(e) => void handleAddClick(e)}
						class="flex gap-1.5 items-center"
					>
						<IconLucidePlus class="size-3.5" />
						Add
					</Button>
				</>
			}
		>
			<SectionCard padded>
				<Show when={hasMissingDefaultExclusions()}>
					<div class="mb-3 rounded-lg border border-amber-6 bg-amber-3/30 px-3 py-2.5">
						<div class="flex items-start gap-2">
							<IconLucideAlertTriangle class="mt-0.5 size-4 shrink-0 text-amber-11" />
							<div class="min-w-0 flex-1 space-y-1">
								<p class="text-xs font-medium text-amber-11">
									Recommended Shelf windows are not excluded
								</p>
								<p class="text-[10px] leading-snug text-amber-11">
									Camera, settings, or recording windows can appear as black
									boxes in screen recordings. Missing: {missingDefaultLabels()}.
								</p>
							</div>
							<Button
								variant="gray"
								size="sm"
								disabled={props.isLoading}
								onClick={handleResetClick}
								class="shrink-0"
							>
								Restore
							</Button>
						</div>
					</div>
				</Show>
				<Show when={!props.isLoading} fallback={<ExcludedWindowsSkeleton />}>
					<Show
						when={hasExclusions()}
						fallback={
							<p class="text-xs text-gray-10">
								No windows are currently excluded.
							</p>
						}
					>
						<div class="flex flex-wrap gap-2">
							<For each={props.excludedWindows}>
								{(entry, index) => (
									<div class="flex gap-2 items-center pr-1 pl-3 py-1.5 rounded-full border bg-gray-3 border-gray-4">
										<div class="flex flex-col leading-tight">
											<span class="text-xs text-gray-12">
												{getExclusionPrimaryLabel(entry)}
											</span>
											<Show when={getExclusionSecondaryLabel(entry)}>
												{(label) => (
													<span class="text-[10px] text-gray-9">{label()}</span>
												)}
											</Show>
										</div>
										<button
											type="button"
											class="flex justify-center items-center rounded-full transition-colors size-5 text-gray-10 hover:bg-gray-5 hover:text-gray-12"
											onClick={() => void props.onRemove(index())}
											aria-label="Remove excluded window"
										>
											<IconLucideX class="size-3" />
										</button>
									</div>
								)}
							</For>
						</div>
					</Show>
				</Show>
			</SectionCard>
		</Section>
	);
}

function ExcludedWindowsSkeleton() {
	const chipWidths = ["w-28", "w-24", "w-32"] as const;

	return (
		<div class="flex flex-wrap gap-2" aria-hidden="true">
			<For each={chipWidths}>
				{(width) => (
					<div class="flex gap-2 items-center pr-1 pl-3 py-1.5 rounded-full border bg-gray-3 border-gray-4 animate-pulse">
						<div class="flex flex-col gap-1 leading-tight">
							<div class={cx("h-2.5 rounded-sm bg-gray-4", width)} />
							<div class="w-14 h-2 rounded-sm bg-gray-4" />
						</div>
						<div class="rounded-full size-5 bg-gray-4" />
					</div>
				)}
			</For>
		</div>
	);
}
