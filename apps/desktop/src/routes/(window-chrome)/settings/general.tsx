import { useNavigate } from "@solidjs/router";
import {
	isPermissionGranted,
	requestPermission,
} from "@tauri-apps/plugin-notification";
import { type OsType, type } from "@tauri-apps/plugin-os";
import "@total-typescript/ts-reset/filter-boolean";
import { cx } from "cva";
import { createEffect, createMemo, createResource, For, Show } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import themePreviewAuto from "~/assets/theme-previews/auto.jpg";
import themePreviewDark from "~/assets/theme-previews/dark.jpg";
import themePreviewLight from "~/assets/theme-previews/light.jpg";
import { generalSettingsStore } from "~/store";
import {
	deriveGeneralSettings,
	type GeneralSettingsStore,
} from "~/utils/general-settings";
import { type AppTheme, commands, type UpdateChannel } from "~/utils/tauri";
import {
	createSettingsSectionScroll,
	Section,
	SectionCard,
	SectionRows,
	SegmentedControl,
	SettingsPageContent,
	ToggleSettingItem,
} from "./Setting";

export default function GeneralSettings() {
	const [store] = createResource(() => generalSettingsStore.get());

	return (
		<Show when={store.state === "ready" && ([store()] as const)}>
			{(store) => <Inner initialStore={store()[0] ?? null} />}
		</Show>
	);
}

function AppearanceSection(props: {
	currentTheme: AppTheme;
	onThemeChange: (theme: AppTheme) => void;
}) {
	const options = [
		{ id: "system", name: "System" },
		{ id: "light", name: "Light" },
		{ id: "dark", name: "Dark" },
	] satisfies { id: AppTheme; name: string }[];

	const previews = {
		system: themePreviewAuto,
		light: themePreviewLight,
		dark: themePreviewDark,
	};

	return (
		<Section
			title="Appearance"
			description="Match Shelf to your system theme or pick a fixed look."
		>
			<SectionCard padded>
				<div
					class="grid grid-cols-3 gap-3"
					onContextMenu={(e) => e.preventDefault()}
				>
					<For each={options}>
						{(theme) => {
							const isSelected = () => props.currentTheme === theme.id;
							return (
								<button
									type="button"
									aria-checked={isSelected()}
									aria-label={`Select theme: ${theme.name}`}
									onClick={() => props.onThemeChange(theme.id)}
									class="flex flex-col gap-2 items-center group focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-9 focus-visible:ring-offset-2 focus-visible:ring-offset-gray-1 rounded-xl"
								>
									<div
										class={cx(
											"w-full aspect-[5/3] rounded-lg overflow-hidden border-2 transition-[border-color,box-shadow] duration-150",
											isSelected()
												? "border-blue-9"
												: "border-gray-4 group-hover:border-gray-6",
										)}
									>
										<Show when={previews[theme.id]} keyed>
											{(preview) => (
												<img
													class="object-cover w-full h-full animate-in fade-in duration-200"
													draggable={false}
													src={preview}
													alt={`Preview of ${theme.name} theme`}
												/>
											)}
										</Show>
									</div>
									<span
										class={cx(
											"text-xs font-medium transition-colors",
											isSelected() ? "text-gray-12" : "text-gray-10",
										)}
									>
										{theme.name}
									</span>
								</button>
							);
						}}
					</For>
				</div>
			</SectionCard>
		</Section>
	);
}

function Inner(props: { initialStore: GeneralSettingsStore | null }) {
	const [settings, setSettings] = createStore<GeneralSettingsStore>(
		deriveGeneralSettings(props.initialStore),
	);

	createEffect(() => {
		setSettings(reconcile(deriveGeneralSettings(props.initialStore)));
	});

	let scrollContainerRef: HTMLDivElement | undefined;
	const navigate = useNavigate();

	createSettingsSectionScroll({
		page: "general",
		container: () => scrollContainerRef,
		navigate: (page) => navigate(`/settings/${page}`),
	});

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

	const ostype: OsType = type();

	return (
		<div
			ref={scrollContainerRef}
			class="cap-settings-page flex flex-col h-full custom-scroll"
		>
			<SettingsPageContent>
				<AppearanceSection
					currentTheme={settings.theme ?? "system"}
					onThemeChange={(newTheme) => {
						setSettings("theme", newTheme);
						generalSettingsStore.set({ theme: newTheme });
					}}
				/>

				{ostype === "macos" && (
					<Section
						title="App"
						description="Choose how Shelf shows up on your system."
					>
						<SectionRows>
							<ToggleSettingItem
								label="Always show dock icon"
								description="Keep Shelf in the dock even when no windows are open."
								value={!settings.hideDockIcon}
								onChange={(v) => handleChange("hideDockIcon", !v)}
							/>
							<ToggleSettingItem
								label="System notifications"
								description="Show notifications for clipboard copies, saved files, and more. You may need to allow Shelf in your system's notification settings."
								value={!!settings.enableNotifications}
								onChange={async (value) => {
									if (value) {
										const permissionGranted = await isPermissionGranted();
										if (!permissionGranted) {
											const permission = await requestPermission();
											if (permission !== "granted") return;
										}
									}
									handleChange("enableNotifications", value);
								}}
							/>
						</SectionRows>
					</Section>
				)}

				<UpdatesSection
					value={settings.updateChannel ?? "stable"}
					onChange={async (channel) => {
						await handleChange("updateChannel", channel);
						try {
							await commands.updatesChannelChanged();
						} catch (error) {
							console.error("Failed to notify update channel change", error);
						}
					}}
				/>
			</SettingsPageContent>
		</div>
	);
}

type UpdateChannelOption = {
	value: UpdateChannel;
	label: string;
	description: string;
};

const UPDATE_CHANNEL_OPTIONS: UpdateChannelOption[] = [
	{
		value: "stable",
		label: "Stable",
		description: "Versioned releases (recommended)",
	},
	{
		value: "nightly",
		label: "Nightly",
		description:
			"The newest builds, updated automatically in the background when you're not recording or exporting. May be unstable.",
	},
];

function UpdatesSection(props: {
	value: UpdateChannel;
	onChange: (value: UpdateChannel) => void;
}) {
	const currentOption = createMemo(
		() =>
			UPDATE_CHANNEL_OPTIONS.find((option) => option.value === props.value) ??
			UPDATE_CHANNEL_OPTIONS[0],
	);

	return (
		<Section
			title="Updates"
			description="Choose which Shelf builds you receive."
		>
			<SectionCard>
				<div class="flex flex-col gap-3 px-4 py-4">
					<div class="flex justify-between items-start gap-4">
						<div class="flex flex-col gap-0.5 min-w-0">
							<p class="text-[13px] text-gray-12">Update channel</p>
							<p class="text-xs leading-snug text-gray-10">
								Which release channel Shelf updates from.
							</p>
						</div>
						<SegmentedControl
							value={props.value}
							onChange={props.onChange}
							options={UPDATE_CHANNEL_OPTIONS.map((option) => ({
								value: option.value,
								label: option.label,
							}))}
						/>
					</div>
					<div class="flex flex-col gap-1.5 px-3 py-2.5 rounded-lg bg-gray-3">
						<p class="text-xs text-gray-12">{currentOption().description}</p>
						<Show when={props.value === "nightly"}>
							<p class="text-[11px] text-gray-10 leading-snug">
								Switching back to Stable will return you to the latest stable
								version, which may be older than your current build.
							</p>
						</Show>
					</div>
				</div>
			</SectionCard>
		</Section>
	);
}
