import { createMemo, createSignal, For, Show } from "solid-js";
import {
	type CameraDeviceSettings,
	CameraSettingsPanel,
	cameraSettingsKeys,
	formatCameraSetting,
	formatMicrophoneSetting,
	type MicrophoneDeviceSettings,
	MicrophoneSettingsPanel,
	type RecordingDeviceSettingsStore,
} from "~/components/recording/DeviceSettingsPanels";
import { generalSettingsStore, recordingSettingsStore } from "~/store";
import {
	type CameraWithDetails,
	createStableDevicesQuery,
	type MicrophoneWithDetails,
} from "~/utils/devices";
import { commands } from "~/utils/tauri";
import IconLucideChevronDown from "~icons/lucide/chevron-down";

import { Section, SectionCard, SettingsPageContent } from "./Setting";

/// The store is typed loosely for these two keys, same as the recording UI does.
const deviceSettingsStore = recordingSettingsStore as unknown as {
	get: () => Promise<RecordingDeviceSettingsStore | undefined>;
	set: (value?: Partial<RecordingDeviceSettingsStore>) => Promise<void>;
	createQuery: () => ReturnType<typeof recordingSettingsStore.createQuery>;
};

export default function DevicesSettings() {
	const devices = createStableDevicesQuery(() => true);
	const settingsQuery = deviceSettingsStore.createQuery();
	const generalSettings = generalSettingsStore.createQuery();

	const deviceSettings = () =>
		(settingsQuery.data ?? undefined) as
			| RecordingDeviceSettingsStore
			| undefined;

	// The compatibility profile caps resolution and frame rate, so the panels
	// warn when a picked format is higher than it can deliver.
	const compatibilityStudioMode = () =>
		generalSettings.data?.studioRecordingQuality === "compatibility";

	const cameraSettingFor = (camera: CameraWithDetails) => {
		const stored = deviceSettings()?.cameraDeviceSettings ?? {};
		for (const key of cameraSettingsKeys(camera)) {
			const value = stored[key];
			if (value) return value;
		}
		return undefined;
	};

	const microphoneSettingFor = (mic: MicrophoneWithDetails) =>
		deviceSettings()?.microphoneDeviceSettings?.[mic.name];

	const saveCameraSettings = async (
		camera: CameraWithDetails,
		settings: CameraDeviceSettings,
	) => {
		const current = (await deviceSettingsStore.get()) ?? {};
		const next = { ...(current.cameraDeviceSettings ?? {}) };
		for (const key of cameraSettingsKeys(camera)) {
			next[key] = settings;
		}
		await deviceSettingsStore.set({ cameraDeviceSettings: next });

		// A running camera preview keeps the old format until it is re-applied.
		const cameraWindowOpen = await commands
			.isCameraWindowOpen()
			.catch(() => false);
		if (!cameraWindowOpen) return;

		const id = camera.model_id
			? { ModelID: camera.model_id }
			: { DeviceID: camera.device_id };
		await commands.setCameraInput(id, true).catch((error) => {
			console.error("Failed to re-apply camera settings:", error);
		});
	};

	const saveMicrophoneSettings = async (
		mic: MicrophoneWithDetails,
		settings: MicrophoneDeviceSettings,
	) => {
		const current = (await deviceSettingsStore.get()) ?? {};
		await deviceSettingsStore.set({
			microphoneDeviceSettings: {
				...(current.microphoneDeviceSettings ?? {}),
				[mic.name]: settings,
			},
		});
	};

	return (
		<SettingsPageContent class="space-y-4">
			<Section
				title="Cameras"
				description="Resolution and frame rate per camera. Auto picks the highest format the camera reports."
			>
				<SectionCard>
					<Show
						when={devices.cameras.length > 0}
						fallback={<EmptyRow label="No cameras found" />}
					>
						<For each={devices.cameras}>
							{(camera) => (
								<DeviceRow
									name={camera.display_name}
									summary={
										cameraSettingFor(camera)
											? formatCameraSetting(cameraSettingFor(camera) ?? {})
											: "Default"
									}
								>
									<CameraSettingsPanel
										camera={camera}
										value={cameraSettingFor(camera)}
										onChange={(settings) =>
											void saveCameraSettings(camera, settings)
										}
										compatibilityStudioMode={compatibilityStudioMode()}
									/>
								</DeviceRow>
							)}
						</For>
					</Show>
				</SectionCard>
			</Section>

			<Section
				title="Microphones"
				description="Sample rate and channels per microphone."
			>
				<SectionCard>
					<Show
						when={devices.microphones.length > 0}
						fallback={<EmptyRow label="No microphones found" />}
					>
						<For each={devices.microphones}>
							{(mic) => (
								<DeviceRow
									name={mic.name}
									summary={
										microphoneSettingFor(mic)
											? formatMicrophoneSetting(microphoneSettingFor(mic) ?? {})
											: "Default"
									}
								>
									<MicrophoneSettingsPanel
										mic={mic}
										value={microphoneSettingFor(mic)}
										onChange={(settings) =>
											void saveMicrophoneSettings(mic, settings)
										}
										compatibilityStudioMode={compatibilityStudioMode()}
									/>
								</DeviceRow>
							)}
						</For>
					</Show>
				</SectionCard>
			</Section>
		</SettingsPageContent>
	);
}

function EmptyRow(props: { label: string }) {
	return <div class="px-4 py-3 text-sm text-gray-10">{props.label}</div>;
}

function DeviceRow(
	props: import("solid-js").ParentProps<{ name: string; summary: string }>,
) {
	const [open, setOpen] = createSignal(false);
	const chevronClass = createMemo(() =>
		open()
			? "size-4 shrink-0 text-gray-10 rotate-180 transition-transform"
			: "size-4 shrink-0 text-gray-10 transition-transform",
	);

	return (
		<div class="border-b border-gray-3 last:border-b-0">
			<button
				type="button"
				onClick={() => setOpen(!open())}
				class="flex gap-3 items-center px-4 py-3 w-full text-left transition-colors hover:bg-gray-3"
				aria-expanded={open()}
			>
				<div class="flex-1 min-w-0">
					<div class="text-sm truncate text-gray-12">{props.name}</div>
					<div class="text-xs text-gray-10">{props.summary}</div>
				</div>
				<IconLucideChevronDown class={chevronClass()} />
			</button>
			<Show when={open()}>
				<div class="px-2 pb-3">{props.children}</div>
			</Show>
		</div>
	);
}
