import { createMutation, createQuery } from "@tanstack/solid-query";
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
import {
	RecordingOptionsProvider,
	useRecordingOptions,
} from "~/routes/(window-chrome)/OptionsContext";
import { generalSettingsStore, recordingSettingsStore } from "~/store";
import {
	type CameraWithDetails,
	createStableDevicesQuery,
	type MicrophoneWithDetails,
} from "~/utils/devices";
import { createCameraMutation, isSystemAudioSupported } from "~/utils/queries";
import { commands, type DeviceOrModelID } from "~/utils/tauri";
import IconLucideChevronDown from "~icons/lucide/chevron-down";

import {
	Section,
	SectionCard,
	SectionRows,
	SelectSettingItem,
	SettingsPageContent,
	ToggleSettingItem,
} from "./Setting";

/// The store is typed loosely for these two keys, same as the recording UI does.
const deviceSettingsStore = recordingSettingsStore as unknown as {
	get: () => Promise<
		| (RecordingDeviceSettingsStore & { cameraId?: DeviceOrModelID | null })
		| undefined
	>;
	set: (value?: Partial<RecordingDeviceSettingsStore>) => Promise<void>;
	createQuery: () => ReturnType<typeof recordingSettingsStore.createQuery>;
};

export default function DevicesSettings() {
	// The picker for the active devices used to live in the tray menu. It needs
	// the shared recording options, which the settings window does not provide
	// on its own.
	return (
		<RecordingOptionsProvider>
			<DevicesSettingsPage />
		</RecordingOptionsProvider>
	);
}

function DevicesSettingsPage() {
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

	// Saving is read/modify/write on one shared blob. Two quick clicks would
	// otherwise interleave and the slower one would write back the older value,
	// so every write waits for the one before it.
	let writeQueue: Promise<unknown> = Promise.resolve();
	const queueWrite = <T,>(write: () => Promise<T>): Promise<T> => {
		const next = writeQueue.then(write, write);
		writeQueue = next.catch(() => undefined);
		return next;
	};

	const saveCameraSettings = (
		camera: CameraWithDetails,
		settings: CameraDeviceSettings,
	) =>
		queueWrite(async () => {
			const current = (await deviceSettingsStore.get()) ?? {};
			const next = { ...(current.cameraDeviceSettings ?? {}) };
			for (const key of cameraSettingsKeys(camera)) {
				next[key] = settings;
			}
			await deviceSettingsStore.set({ cameraDeviceSettings: next });

			// A running camera preview keeps the old format until it is re-applied.
			// Only the camera that is actually selected gets re-applied: editing
			// an idle camera must not take over the live preview.
			const selectedId = (await deviceSettingsStore.get())?.cameraId;
			const isSelected =
				selectedId != null &&
				("ModelID" in selectedId
					? selectedId.ModelID === camera.model_id
					: selectedId.DeviceID === camera.device_id);
			if (!isSelected) return;

			const cameraWindowOpen = await commands
				.isCameraWindowOpen()
				.catch(() => false);
			if (!cameraWindowOpen) return;

			await commands.setCameraInput(selectedId, true).catch((error) => {
				console.error("Failed to re-apply camera settings:", error);
			});
		});

	const saveMicrophoneSettings = (
		mic: MicrophoneWithDetails,
		settings: MicrophoneDeviceSettings,
	) =>
		queueWrite(async () => {
			const current = (await deviceSettingsStore.get()) ?? {};
			await deviceSettingsStore.set({
				microphoneDeviceSettings: {
					...(current.microphoneDeviceSettings ?? {}),
					[mic.name]: settings,
				},
			});
		});

	return (
		<SettingsPageContent class="space-y-4">
			<Section
				title="In use"
				description="What a recording picks up. The same choice applies to the recording overlay."
			>
				<ActiveDevices
					cameras={devices.cameras}
					microphones={devices.microphones}
				/>
			</Section>

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

const NO_DEVICE = "";

/** Camera, microphone and system audio: what the next recording will capture. */
function ActiveDevices(props: {
	cameras: CameraWithDetails[];
	microphones: MicrophoneWithDetails[];
}) {
	const { rawOptions, setOptions } = useRecordingOptions();
	const setCamera = createCameraMutation();
	const systemAudioSupported = createQuery(() => isSystemAudioSupported);

	const setMicrophone = createMutation(() => ({
		mutationFn: async (name: string | null) => {
			const previous = rawOptions.micName ?? null;
			if (previous !== name) setOptions("micName", name);
			try {
				await commands.setMicInput(name);
			} catch (error) {
				if (previous !== name) setOptions("micName", previous);
				throw error;
			}
		},
		onError: (error) => console.error("Failed to set microphone:", error),
	}));

	const DISCONNECTED = "__disconnected__";

	const selectedCamera = () => {
		const selected = rawOptions.cameraID;
		if (!selected) return undefined;
		return props.cameras.find((camera) =>
			"ModelID" in selected
				? camera.model_id === selected.ModelID
				: camera.device_id === selected.DeviceID,
		);
	};

	// A camera that is selected but unplugged keeps its slot: reading as "None"
	// would suggest the selection is gone, and it is not.
	const cameraDisconnected = () =>
		!!rawOptions.cameraID && selectedCamera() === undefined;

	const cameraValue = () => {
		if (cameraDisconnected()) return DISCONNECTED;
		return selectedCamera()?.device_id ?? NO_DEVICE;
	};

	const cameraOptions = () => [
		{ text: "None", value: NO_DEVICE },
		...props.cameras.map((camera) => ({
			text: camera.display_name,
			value: camera.device_id,
		})),
		...(cameraDisconnected()
			? [{ text: "Selected camera, not connected", value: DISCONNECTED }]
			: []),
	];

	const microphoneOptions = () => [
		{ text: "None", value: NO_DEVICE },
		...props.microphones.map((mic) => ({ text: mic.name, value: mic.name })),
	];

	const selectCamera = (deviceId: string) => {
		// Picking the placeholder again changes nothing; the device is not there.
		if (deviceId === DISCONNECTED) return;
		if (deviceId === NO_DEVICE) {
			setCamera.mutate({ model: null });
			return;
		}
		const camera = props.cameras.find((it) => it.device_id === deviceId);
		if (!camera) return;
		setCamera.mutate({
			model: camera.model_id
				? { ModelID: camera.model_id }
				: { DeviceID: camera.device_id },
		});
	};

	return (
		<SectionRows>
			<SelectSettingItem
				label="Camera"
				description="Shown as an overlay while recording."
				value={cameraValue()}
				options={cameraOptions()}
				onChange={selectCamera}
			/>
			<SelectSettingItem
				label="Microphone"
				value={rawOptions.micName ?? NO_DEVICE}
				options={microphoneOptions()}
				onChange={(name) =>
					setMicrophone.mutate(name === NO_DEVICE ? null : name)
				}
			/>
			<ToggleSettingItem
				label="System audio"
				description={
					systemAudioSupported.data === false
						? "Needs macOS 13 or later."
						: "Records what the Mac plays back."
				}
				value={
					systemAudioSupported.data !== false && !!rawOptions.captureSystemAudio
				}
				onChange={(value) => {
					if (systemAudioSupported.data === false) return;
					setOptions({ captureSystemAudio: value });
				}}
			/>
		</SectionRows>
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
