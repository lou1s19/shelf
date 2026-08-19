import { cx } from "cva";
import { createMemo, For, Show } from "solid-js";
import type { CameraWithDetails, MicrophoneWithDetails } from "~/utils/devices";
import IconLucideCheck from "~icons/lucide/check";

export type CameraDeviceSettings = {
	width?: number;
	height?: number;
	frameRate?: number;
};

export type MicrophoneDeviceSettings = {
	sampleRate?: number;
	channels?: number;
};

export type RecordingDeviceSettingsStore = {
	cameraDeviceSettings?: Record<string, CameraDeviceSettings>;
	microphoneDeviceSettings?: Record<string, MicrophoneDeviceSettings>;
};

/// A camera is stored under both keys so a device that reports a model id is
/// still found when only the device id is known.
export const cameraSettingsKeys = (camera: CameraWithDetails) => [
	`device:${camera.device_id}`,
	...(camera.model_id ? [`model:${camera.model_id}`] : []),
];

export const formatCameraSetting = (format: CameraDeviceSettings) => {
	const size =
		format.width && format.height ? `${format.width}×${format.height}` : "Auto";
	const rate = format.frameRate ? `${Math.round(format.frameRate)}fps` : "Auto";
	return `${size} @ ${rate}`;
};

export const formatMicrophoneSetting = (setting: MicrophoneDeviceSettings) => {
	const rate = setting.sampleRate ? `${setting.sampleRate / 1000}kHz` : "Auto";
	const channels =
		setting.channels === 1
			? "Mono"
			: setting.channels === 2
				? "Stereo"
				: setting.channels
					? `${setting.channels}ch`
					: "Auto";
	return `${rate} ${channels}`;
};

export const isHighCameraSetting = (setting: CameraDeviceSettings) =>
	(setting.width ?? 0) >= 3840 ||
	(setting.height ?? 0) >= 2160 ||
	(setting.frameRate ?? 0) > 30;

export const isHighMicrophoneSetting = (setting: MicrophoneDeviceSettings) =>
	(setting.sampleRate ?? 0) > 48_000 || (setting.channels ?? 0) > 2;

export function CameraSettingsPanel(props: {
	camera: CameraWithDetails;
	value?: CameraDeviceSettings;
	onChange: (settings: CameraDeviceSettings) => void;
	compatibilityStudioMode: boolean;
}) {
	const formats = createMemo(() => {
		const formats = props.camera.formats ?? [];
		const seen = new Set<string>();
		return formats
			.filter((format) => {
				const key = `${format.width}:${format.height}:${Math.round(format.frameRate)}`;
				if (seen.has(key)) return false;
				seen.add(key);
				return true;
			})
			.sort(
				(a, b) =>
					b.width * b.height - a.width * a.height || b.frameRate - a.frameRate,
			);
	});

	const defaultSetting = createMemo<CameraDeviceSettings | undefined>(() => {
		if (props.camera.bestFormat) {
			const { width, height, frameRate } = props.camera.bestFormat;
			return { width, height, frameRate };
		}
		const first = formats()[0];
		if (!first) return undefined;
		return {
			width: first.width,
			height: first.height,
			frameRate: first.frameRate,
		};
	});

	const isDefaultSelected = () => {
		const value = props.value;
		return (
			!value ||
			(value.width === undefined &&
				value.height === undefined &&
				value.frameRate === undefined)
		);
	};

	const isSelected = (format: CameraDeviceSettings) =>
		props.value?.width === format.width &&
		props.value?.height === format.height &&
		Math.round(props.value?.frameRate ?? 0) ===
			Math.round(format.frameRate ?? 0);

	return (
		<div class="flex flex-col gap-1">
			<button
				type="button"
				onClick={() => props.onChange({})}
				class={cx(
					"flex items-center gap-3 px-3 py-2.5 rounded-lg text-left text-sm outline-none transition-colors",
					isDefaultSelected()
						? "bg-blue-500 text-white"
						: "text-gray-12 hover:bg-gray-4",
				)}
			>
				<div class="flex-1 min-w-0">
					<div class="truncate">Default</div>
					<Show when={defaultSetting()}>
						{(setting) => (
							<div
								class={cx(
									"text-[11px] truncate",
									isDefaultSelected() ? "text-white/70" : "text-gray-10",
								)}
							>
								{formatCameraSetting(setting())}
							</div>
						)}
					</Show>
				</div>
				<Show when={isDefaultSelected()}>
					<IconLucideCheck class="size-4 shrink-0" />
				</Show>
			</button>
			<Show when={formats().length > 0}>
				<For each={formats()}>
					{(format) => {
						const setting = () => ({
							width: format.width,
							height: format.height,
							frameRate: format.frameRate,
						});
						const high = () => isHighCameraSetting(setting());
						return (
							<button
								type="button"
								onClick={() => props.onChange(setting())}
								class={cx(
									"flex items-center gap-3 px-3 py-2.5 rounded-lg text-left text-sm outline-none transition-colors",
									isSelected(setting())
										? "bg-blue-500 text-white"
										: "text-gray-12 hover:bg-gray-4",
								)}
							>
								<div class="flex-1 min-w-0">
									<div class="truncate">{formatCameraSetting(setting())}</div>
									<Show when={props.compatibilityStudioMode && high()}>
										<div
											class={cx(
												"text-[11px]",
												isSelected(setting())
													? "text-white/70"
													: "text-amber-11",
											)}
										>
											Compatibility mode may reduce this setting.
										</div>
									</Show>
								</div>
								<Show when={isSelected(setting())}>
									<IconLucideCheck class="size-4 shrink-0" />
								</Show>
							</button>
						);
					}}
				</For>
			</Show>
		</div>
	);
}

export function MicrophoneSettingsPanel(props: {
	mic: MicrophoneWithDetails;
	value?: MicrophoneDeviceSettings;
	onChange: (settings: MicrophoneDeviceSettings) => void;
	compatibilityStudioMode: boolean;
}) {
	const formats = createMemo(() => {
		const formats =
			props.mic.formats && props.mic.formats.length > 0
				? props.mic.formats
				: props.mic.sampleRate && props.mic.channels
					? [{ sampleRate: props.mic.sampleRate, channels: props.mic.channels }]
					: [];
		const seen = new Set<string>();
		return formats
			.filter((format) => {
				const key = `${format.sampleRate}:${format.channels}`;
				if (seen.has(key)) return false;
				seen.add(key);
				return true;
			})
			.sort((a, b) => b.sampleRate - a.sampleRate || b.channels - a.channels);
	});

	const defaultSetting = createMemo<MicrophoneDeviceSettings | undefined>(
		() => {
			if (props.mic.sampleRate && props.mic.channels) {
				return {
					sampleRate: props.mic.sampleRate,
					channels: props.mic.channels,
				};
			}
			const first = formats()[0];
			if (!first) return undefined;
			return { sampleRate: first.sampleRate, channels: first.channels };
		},
	);

	const isDefaultSelected = () => {
		const value = props.value;
		return (
			!value || (value.sampleRate === undefined && value.channels === undefined)
		);
	};

	const isSelected = (format: MicrophoneDeviceSettings) =>
		props.value?.sampleRate === format.sampleRate &&
		props.value?.channels === format.channels;

	return (
		<div class="flex flex-col gap-1">
			<button
				type="button"
				onClick={() => props.onChange({})}
				class={cx(
					"flex items-center gap-3 px-3 py-2.5 rounded-lg text-left text-sm outline-none transition-colors",
					isDefaultSelected()
						? "bg-blue-500 text-white"
						: "text-gray-12 hover:bg-gray-4",
				)}
			>
				<div class="flex-1 min-w-0">
					<div class="truncate">Default</div>
					<Show when={defaultSetting()}>
						{(setting) => (
							<div
								class={cx(
									"text-[11px] truncate",
									isDefaultSelected() ? "text-white/70" : "text-gray-10",
								)}
							>
								{formatMicrophoneSetting(setting())}
							</div>
						)}
					</Show>
				</div>
				<Show when={isDefaultSelected()}>
					<IconLucideCheck class="size-4 shrink-0" />
				</Show>
			</button>
			<Show when={formats().length > 0}>
				<For each={formats()}>
					{(format) => {
						const setting = () => ({
							sampleRate: format.sampleRate,
							channels: format.channels,
						});
						const high = () => isHighMicrophoneSetting(setting());
						return (
							<button
								type="button"
								onClick={() => props.onChange(setting())}
								class={cx(
									"flex items-center gap-3 px-3 py-2.5 rounded-lg text-left text-sm outline-none transition-colors",
									isSelected(setting())
										? "bg-blue-500 text-white"
										: "text-gray-12 hover:bg-gray-4",
								)}
							>
								<div class="flex-1 min-w-0">
									<div class="truncate">
										{formatMicrophoneSetting(setting())}
									</div>
									<Show when={props.compatibilityStudioMode && high()}>
										<div
											class={cx(
												"text-[11px]",
												isSelected(setting())
													? "text-white/70"
													: "text-amber-11",
											)}
										>
											Compatibility mode may reduce this setting.
										</div>
									</Show>
								</div>
								<Show when={isSelected(setting())}>
									<IconLucideCheck class="size-4 shrink-0" />
								</Show>
							</button>
						);
					}}
				</For>
			</Show>
		</div>
	);
}
