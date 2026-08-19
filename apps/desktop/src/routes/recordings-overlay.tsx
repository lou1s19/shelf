import { Button } from "@cap/ui-solid";
import Tooltip from "@corvu/tooltip";
import { createElementBounds } from "@solid-primitives/bounds";
import { makePersisted } from "@solid-primitives/storage";
import { createMutation, createQuery } from "@tanstack/solid-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { cx } from "cva";
import {
	type Accessor,
	type ComponentProps,
	createEffect,
	createMemo,
	createResource,
	createSignal,
	For,
	Match,
	onCleanup,
	onMount,
	Show,
	Suspense,
	Switch,
} from "solid-js";
import { createStore, produce, type SetStoreFunction } from "solid-js/store";
import { TransitionGroup } from "solid-transition-group";
import { generalSettingsStore } from "~/store";
import { createTauriEventListener } from "~/utils/createEventListener";
import { createExportToFileTask, exportVideo } from "~/utils/export";
import { commands, events, type FramesRendered } from "~/utils/tauri";
import IconCapEditor from "~icons/cap/editor";
import IconLucideCheck from "~icons/lucide/check";
import IconLucideClock from "~icons/lucide/clock";
import IconLucideCopy from "~icons/lucide/copy";
import IconLucideEye from "~icons/lucide/eye";
import { FPS, OUTPUT_SIZE } from "./editor/context";

type MediaType = "recording" | "screenshot";

type MediaEntry = {
	path: string;
	prettyName: string;
	/** Epoch ms. Drives the auto hide timer and decides which card is dropped first. */
	createdAt: number;
	type?: MediaType;
	/** Small JPEG data URL shown until the real file has been written. */
	preview?: string;
};

const CARD_HEIGHT = 150;
const CARD_GAP = 16;
const STACK_TOP_PADDING = 48;
const STACK_BOTTOM_PADDING = 80;
/** Safety net for very tall screens: nobody needs a wall of pins. */
const MAX_VISIBLE_CARDS = 12;

/** How many cards fit on this screen without the stack running off the top edge. */
export function maxVisibleCards(availableHeight: number): number {
	const usable = availableHeight - STACK_TOP_PADDING - STACK_BOTTOM_PADDING;
	const fits = Math.floor((usable + CARD_GAP) / (CARD_HEIGHT + CARD_GAP));
	return Math.max(1, Math.min(MAX_VISIBLE_CARDS, fits));
}

/**
 * `null` means a pin stays until it is dismissed or acted on, which is the
 * default: a pin that vanishes on its own takes the screenshot with it.
 */
export function autoHideMsFromSettings(
	seconds: number | null | undefined,
): number | null {
	return typeof seconds === "number" && seconds > 0 ? seconds * 1000 : null;
}

export default function () {
	onMount(() => {
		document.documentElement.setAttribute("data-transparent-window", "true");
		document.body.style.background = "transparent";
	});

	const [recordings, setRecordings] = makePersisted(
		createStore<MediaEntry[]>([]),
		{ name: "recordings-store" },
	);
	const [screenshots, setScreenshots] = makePersisted(
		createStore<MediaEntry[]>([]),
		{ name: "screenshots-store" },
	);

	const settings = generalSettingsStore.createQuery();

	const screenshotAutoHideMs = () =>
		autoHideMsFromSettings(settings.data?.screenshotPinAutoHideSeconds);

	// Cmd+C over a card. Measured on a mixed-DPI multi-monitor setup: while the
	// pointer rests on a card, the hit test in fake_window.rs flaps, so the panel
	// loses focus and the webview sees mouseleave for a moment even though the
	// mouse never moved. Releasing on either of those disarmed the shortcut
	// between two keystrokes. The grab is therefore held for a grace period and
	// dropped by a timer, not by the first leave.
	const HOLD_AFTER_LEAVE_MS = 3000;
	const MAX_HOLD_MS = 20000;

	const [hoveredCopy, setHoveredCopy] = createSignal<(() => void) | null>(null);
	const [newestCopy, setNewestCopy] = createSignal<(() => void) | null>(null);
	let copyShortcutArmed = false;
	let releaseTimer: ReturnType<typeof setTimeout> | undefined;
	let maxHoldTimer: ReturnType<typeof setTimeout> | undefined;

	const setCopyShortcut = (active: boolean) => {
		if (copyShortcutArmed === active) return;
		copyShortcutArmed = active;
		commands
			.setPinCopyShortcutActive(active)
			.catch((error) =>
				console.error("Failed to change the copy shortcut", error),
			);
	};

	/** The hovered card, or the newest one when the pointer never reached a card. */
	const copyTarget = () => hoveredCopy() ?? newestCopy();

	const clearTimers = () => {
		if (releaseTimer !== undefined) clearTimeout(releaseTimer);
		if (maxHoldTimer !== undefined) clearTimeout(maxHoldTimer);
		releaseTimer = undefined;
		maxHoldTimer = undefined;
	};

	const releaseCopyShortcut = () => {
		clearTimers();
		setHoveredCopy(null);
		setCopyShortcut(false);
	};

	const armCopyShortcut = (run?: () => void) => {
		if (run) setHoveredCopy(() => run);
		if (!copyTarget()) return;

		if (releaseTimer !== undefined) {
			clearTimeout(releaseTimer);
			releaseTimer = undefined;
		}
		setCopyShortcut(true);
		// Only started once. Restarting it on every re-arm would let the flapping
		// hit test push the cap ahead of itself forever.
		if (maxHoldTimer === undefined)
			maxHoldTimer = setTimeout(releaseCopyShortcut, MAX_HOLD_MS);
	};

	/** Not an immediate release: a flapping hit test would disarm mid-keystroke. */
	const scheduleRelease = () => {
		if (!copyShortcutArmed) return;
		if (releaseTimer !== undefined) clearTimeout(releaseTimer);
		releaseTimer = setTimeout(releaseCopyShortcut, HOLD_AFTER_LEAVE_MS);
	};

	const runCopy = () => {
		const run = copyTarget();
		if (!run) return;
		releaseCopyShortcut();
		run();
	};

	createTauriEventListener(events.onPinCopyPress, runCopy);

	const onWindowFocus = () => armCopyShortcut();
	const onKeyDown = (event: KeyboardEvent) => {
		if (event.key !== "c" || !(event.metaKey || event.ctrlKey)) return;
		event.preventDefault();
		runCopy();
	};
	const onVisibilityChange = () => {
		if (document.hidden) releaseCopyShortcut();
	};

	window.addEventListener("focus", onWindowFocus);
	window.addEventListener("keydown", onKeyDown);
	document.addEventListener("visibilitychange", onVisibilityChange);
	onCleanup(() => {
		window.removeEventListener("focus", onWindowFocus);
		window.removeEventListener("keydown", onKeyDown);
		document.removeEventListener("visibilitychange", onVisibilityChange);
		releaseCopyShortcut();
	});

	const removeEntry = (path: string, type: MediaType) => {
		const setMedia = type === "screenshot" ? setScreenshots : setRecordings;
		setMedia(
			produce((state) => {
				const index = state.findIndex((entry) => entry.path === path);
				if (index !== -1) state.splice(index, 1);
			}),
		);
	};

	// The stack grows upwards from the bottom left corner. Anything that no longer fits on this
	// screen would sit above the top edge, out of reach, so the oldest cards are dropped instead.
	const trimToScreen = () => {
		const max = maxVisibleCards(window.innerHeight);
		const entries = [
			...recordings.map((entry) => ({
				path: entry.path,
				type: (entry.type ?? "recording") as MediaType,
				createdAt: entry.createdAt ?? 0,
			})),
			...screenshots.map((entry) => ({
				path: entry.path,
				type: (entry.type ?? "screenshot") as MediaType,
				createdAt: entry.createdAt ?? 0,
			})),
		];

		if (entries.length <= max) return;

		entries.sort((a, b) => a.createdAt - b.createdAt);
		for (const entry of entries.slice(0, entries.length - max))
			removeEntry(entry.path, entry.type);
	};

	// A pin is announced twice: once with a preview while the file is still being written, once
	// when it exists. Remembering what was already shown keeps the second announcement from
	// resurrecting a card the user has dismissed in between.
	const seenPaths = new Set<string>();

	const addMediaEntry = (path: string, type?: MediaType, preview?: string) => {
		if (seenPaths.has(path)) return;
		seenPaths.add(path);

		const setMedia = type === "screenshot" ? setScreenshots : setRecordings;
		setMedia(
			produce((state) => {
				if (state.some((entry) => entry.path === path)) return;
				const fileName = path.split("/").pop() || "";
				const match = fileName.match(
					/(?:Shelf|Cap) (\d{4}-\d{2}-\d{2} at \d{2}\.\d{2}\.\d{2})/,
				);
				const prettyName = match ? match[1].replace(/\./g, ":") : fileName;
				state.unshift({
					path,
					prettyName,
					createdAt: Date.now(),
					type,
					preview,
				});
			}),
		);

		trimToScreen();
	};

	/** Drops the preview so the card starts showing the file itself. */
	const markFileWritten = (path: string) => {
		setScreenshots(
			produce((state) => {
				const index = state.findIndex((entry) => entry.path === path);
				if (index !== -1) state[index].preview = undefined;
			}),
		);
	};

	const onResize = () => trimToScreen();
	window.addEventListener("resize", onResize);
	onCleanup(() => window.removeEventListener("resize", onResize));

	createTauriEventListener(events.newStudioRecordingAdded, (payload) => {
		addMediaEntry(payload.path, "recording");
	});

	createTauriEventListener(events.screenshotPinPreview, (payload) => {
		addMediaEntry(payload.path, "screenshot", payload.preview);
	});

	createTauriEventListener(events.newScreenshotAdded, (payload) => {
		if (seenPaths.has(payload.path)) {
			markFileWritten(payload.path);
			return;
		}

		// A screenshot only pins when pinning is what the user asked for. Otherwise the overlay
		// may still be open with older cards, and the new one has no business showing up there.
		if (
			!settings.isPending &&
			(settings.data?.postScreenshotBehaviour ?? "showOverlay") !==
				"showOverlay"
		)
			return;

		addMediaEntry(payload.path, "screenshot");
	});

	// Pins from an earlier session would outlive their own hide time, so they are dropped on
	// start. Without auto hiding they are meant to stay, and they do.
	let startupCleanupDone = false;
	createEffect(() => {
		if (settings.isPending || startupCleanupDone) return;
		startupCleanupDone = true;

		const ms = screenshotAutoHideMs();
		if (ms === null) return;

		const cutoff = Date.now() - ms;
		setScreenshots(
			produce((state) => {
				for (let index = state.length - 1; index >= 0; index--)
					if ((state[index].createdAt ?? 0) <= cutoff) state.splice(index, 1);
			}),
		);
	});

	const allMedia = createMemo(() => [...recordings, ...screenshots]);

	// Nothing left to show means nothing left to keep open. The window is only closed once it
	// actually held something, so the empty window a screenshot is about to fill stays alive.
	const [hadMedia, setHadMedia] = createSignal(false);
	createEffect(() => {
		if (allMedia().length > 0) {
			setHadMedia(true);
			return;
		}

		if (!hadMedia()) return;

		const timeout = setTimeout(() => {
			if (allMedia().length === 0)
				commands
					.closeRecordingsOverlayWindow()
					.catch((error) => console.error("Failed to close overlay", error));
		}, 400);
		onCleanup(() => clearTimeout(timeout));
	});

	return (
		<div
			class="w-screen h-screen bg-transparent relative overflow-y-hidden"
			style={{
				"scrollbar-color": "auto transparent",
			}}
		>
			<div class="w-full relative left-0 bottom-0 flex flex-col-reverse pl-[40px] pb-[80px] gap-4 h-full overflow-hidden">
				<div class="flex flex-col gap-4 pt-12 w-full">
					<TransitionGroup
						enterToClass="translate-y-0"
						enterClass="opacity-0 translate-y-4"
						exitToClass="opacity-0 -translate-x-1/2 ease-out"
						exitClass="opacity-100 translate-x-0"
						exitActiveClass="absolute"
					>
						<For each={allMedia()}>
							{(media) => {
								const [ref, setRef] = createSignal<HTMLElement | null>(null);

								const type = media.type ?? "recording";
								const isRecording = type !== "screenshot";

								const { copy, save, actionState } =
									createRecordingMutations(media);

								const [metadata] = createResource(async () => {
									if (!isRecording) return null;

									const result = await commands
										.getVideoMetadata(media.path)
										.catch((e) => {
											console.error(`Failed to get metadata: ${e}`);
										});
									if (!result) return;

									const { duration, size } = result;
									// Calculate estimated export time (rough estimation: 1.5x real-time for 1080p)
									const estimatedExportTime = Math.ceil(duration * 1.5);
									console.log(
										`Metadata for ${media.path}: duration=${duration}, size=${size}, estimatedExport=${estimatedExportTime}`,
									);

									return { duration, size, estimatedExportTime };
								});

								const [imageExists, setImageExists] = createSignal(true);

								const isLoading = () => copy.isPending || save.isPending;

								createFakeWindowBounds(ref, () => media.path);

								const dismiss = () => removeEntry(media.path, type);

								// A card that vanishes the instant the key is pressed leaves
								// no sign that anything happened. The tick fades in, holds,
								// fades out, and only then is the card dropped.
								const [copied, setCopied] = createSignal(false);
								const copyThisCard = () =>
									copy.mutate(undefined, {
										onSuccess: () => {
											setCopied(true);
											setTimeout(() => setCopied(false), 700);
											setTimeout(dismiss, 950);
										},
									});

								// The stack grows from the newest entry, so the first card
								// rendered is the one Cmd+C means without a pointer.
								createEffect(() => {
									if (allMedia()[0]?.path === media.path)
										setNewestCopy(() => copyThisCard);
								});
								// Both signals may still point at this card when it is
								// removed by a path that fires no mouseleave: the copy
								// button, a finished export, a drag-out.
								onCleanup(() => {
									setNewestCopy((current) =>
										current === copyThisCard ? null : current,
									);
									if (hoveredCopy() === copyThisCard) releaseCopyShortcut();
								});

								// The pin disappears once the file has landed somewhere else. A drag the
								// user gave up on leaves it alone.
								const dragOut = (e: MouseEvent) => {
									if (isRecording || e.button !== 0) return;
									if (
										e.target instanceof Element &&
										e.target.closest("button, a, input")
									)
										return;

									commands
										.startFileDrag(media.path)
										.then((dropped) => {
											if (dropped) dismiss();
										})
										.catch((error) =>
											console.error("Failed to start drag", error),
										);
								};

								const autoHideMs = () =>
									isRecording ? null : screenshotAutoHideMs();

								let hideTimer: ReturnType<typeof setTimeout> | undefined;
								let hideDeadline = 0;
								let hideRemaining = 0;
								let isHovered = false;

								const clearHideTimer = () => {
									if (hideTimer === undefined) return;
									clearTimeout(hideTimer);
									hideTimer = undefined;
								};

								const scheduleHide = (ms: number) => {
									clearHideTimer();
									hideDeadline = Date.now() + ms;
									hideTimer = setTimeout(dismiss, ms);
								};

								createEffect(() => {
									const ms = autoHideMs();
									if (ms === null) {
										clearHideTimer();
										return;
									}
									if (!isHovered) scheduleHide(ms);
								});

								onCleanup(clearHideTimer);

								// Nothing may vanish from under the cursor, so the timer pauses while the
								// card is hovered and picks up the rest of the time afterwards.
								const pauseHide = () => {
									isHovered = true;
									if (hideTimer === undefined) return;
									hideRemaining = Math.max(0, hideDeadline - Date.now());
									clearHideTimer();
								};

								const resumeHide = () => {
									isHovered = false;
									const ms = autoHideMs();
									if (ms === null) return;
									scheduleHide(hideRemaining > 0 ? hideRemaining : ms);
								};

								return (
									<Suspense>
										<div
											ref={setRef}
											style={{ "border-color": "rgba(255, 255, 255, 0.1)" }}
											class={cx(
												"overflow-hidden relative rounded-xl shadow-sm transition-all duration-200 w-[260px] h-[150px] bg-gray-12 border group",
												!isRecording && "cursor-grab active:cursor-grabbing",
											)}
											onMouseDown={dragOut}
											onMouseEnter={() => {
												pauseHide();
												armCopyShortcut(copyThisCard);
											}}
											onMouseLeave={() => {
												resumeHide();
												scheduleRelease();
											}}
										>
											<div
												class={cx(
													"w-full h-full flex relative bg-transparent z-10 overflow-hidden transition-all",
													isLoading() && "backdrop-blur-sm bg-gray-12",
												)}
												style={{
													"pointer-events": "auto",
												}}
											>
												<Show
													when={imageExists()}
													fallback={
														<div class="absolute inset-0 w-full h-full pointer-events-none -z-10 bg-gray-10" />
													}
												>
													<img
														class="pointer-events-none w-full h-full object-cover absolute inset-0 -z-10 rounded-[7.4px]"
														alt="media preview"
														src={
															media.preview ??
															`${convertFileSrc(
																isRecording
																	? `${media.path}/screenshots/display.jpg`
																	: `${media.path}`,
															)}?t=${Date.now()}`
														}
														onError={() => setImageExists(false)}
													/>
												</Show>

												<Switch>
													<Match
														when={
															isRecording &&
															actionState.type === "copy" &&
															actionState.state
														}
														keyed
													>
														{(state) => (
															<ActionProgressOverlay
																title={
																	state.type === "rendering"
																		? "Rendering video"
																		: state.type === "copying"
																			? "Copying to clipboard"
																			: "Copied to clipboard"
																}
																progressPercentage={actionProgressPercentage(
																	actionState,
																)}
															/>
														)}
													</Match>
													<Match
														when={
															actionState.type === "save" && actionState.state
														}
														keyed
													>
														{(state) => (
															<ActionProgressOverlay
																title={(() => {
																	if (state.type === "choosing-location")
																		return "Preparing";

																	if (isRecording) {
																		if (state.type === "rendering")
																			return "Rendering video";
																		if (state.type === "saving")
																			return "Saving video";
																		return "Saved video";
																	} else {
																		if (state.type === "rendering")
																			return "Rendering image";
																		if (state.type === "saving")
																			return "Saving image";
																		return "Saved image";
																	}
																})()}
																progressPercentage={actionProgressPercentage(
																	actionState,
																)}
																progressMessage={
																	state.type === "choosing-location" &&
																	`Choose where to ${
																		isRecording ? "export video" : "save image"
																	}...`
																}
															/>
														)}
													</Match>
												</Switch>

												<div
													class={cx(
														"flex absolute inset-0 z-30 justify-center items-center rounded-[7.4px] transition-opacity duration-200 pointer-events-none",
														copied() ? "opacity-100" : "opacity-0",
													)}
													style={{
														"background-color": "rgba(0, 0, 0, 0.55)",
														"backdrop-filter": "blur(2px)",
													}}
												>
													<IconLucideCheck class="text-white size-10" />
												</div>

												<div
													style={{
														"background-color": "rgba(0, 0, 0, 0.4)",
													}}
													class={cx(
														"absolute inset-0 transition-all duration-150 pointer-events-auto rounded-[7.4px] dark:text-gray-100",
														"opacity-0 group-hover:opacity-100",
														"backdrop-blur-sm p-2",
													)}
												>
													<TooltipIconButton
														class="absolute top-3 left-3 z-20"
														tooltipText="Close"
														tooltipPlacement="right"
														onClick={dismiss}
													>
														<IconCapCircleX class="size-4" />
													</TooltipIconButton>
													<TooltipIconButton
														class="absolute bottom-3 left-3 z-20"
														tooltipText="Edit"
														tooltipPlacement="right"
														onClick={() => {
															dismiss();
															commands.showWindow(
																isRecording
																	? { Editor: { project_path: media.path } }
																	: { ScreenshotEditor: { path: media.path } },
															);
														}}
													>
														<IconCapEditor class="size-4" />
													</TooltipIconButton>
													<Show when={!isRecording}>
														<TooltipIconButton
															class="absolute top-3 right-3 z-20"
															tooltipText="View"
															tooltipPlacement="left"
															onClick={() => {
																commands.openFilePath(media.path);
															}}
														>
															<IconLucideEye class="size-4" />
														</TooltipIconButton>
													</Show>
													<TooltipIconButton
														class="absolute right-3 bottom-3 z-20"
														tooltipText={copy.isPending ? "Copying" : "Copy"}
														tooltipPlacement="left"
														onClick={copyThisCard}
													>
														<IconLucideCopy class="size-4" />
													</TooltipIconButton>
													<div class="flex absolute inset-0 justify-center items-center">
														<Button
															variant="white"
															size="sm"
															onClick={() =>
																save.mutate(undefined, { onSuccess: dismiss })
															}
														>
															Export
														</Button>
													</div>
												</div>
												<Show when={metadata.latest}>
													{(metadata) => (
														<div
															style={{
																"font-size": "12px",
																"border-end-end-radius": "7.4px",
																"border-end-start-radius": "7.4px",
															}}
															class={cx(
																"absolute bottom-0 left-0 right-0 font-medium text-gray-4 bg-[#00000080] backdrop-blur-lg px-3 py-2 flex justify-between items-center pointer-events-none transition-all max-w-full overflow-hidden",
																isLoading()
																	? "opacity-0"
																	: "group-hover:opacity-0",
															)}
														>
															<span class="flex items-center">
																<IconCapCamera class="w-[16px] h-[16px] mr-1.5" />
																{Math.floor(metadata().duration / 60)}:
																{Math.floor(metadata().duration % 60)
																	.toString()
																	.padStart(2, "0")}
															</span>
															<span class="flex items-center">
																<IconLucideHardDrive class="w-[16px] h-[16px] mr-1.5" />
																{metadata().size.toFixed(2)} MB
															</span>
															<span class="flex items-center">
																<IconLucideClock class="w-[16px] h-[16px] mr-1.5" />
																~
																{Math.floor(
																	metadata().estimatedExportTime / 60,
																)}
																:
																{Math.floor(metadata().estimatedExportTime % 60)
																	.toString()
																	.padStart(2, "0")}
															</span>
														</div>
													)}
												</Show>
											</div>
										</div>
									</Suspense>
								);
							}}
						</For>
					</TransitionGroup>
				</div>
			</div>
		</div>
	);
}

function ActionProgressOverlay(props: {
	title: string;
	// percentage 0-100
	progressPercentage: number;
	progressMessage?: string | false;
}) {
	return (
		<div
			style={{
				"background-color": "rgba(0, 0, 0, 0.85)",
			}}
			class="absolute inset-0 flex items-center justify-center z-999999 pointer-events-auto"
		>
			<div class="w-[80%] text-center">
				<h3 class="mb-3 text-sm font-medium text-gray-1 dark:text-gray-12">
					{props.title}
				</h3>
				<div class="w-full bg-gray-10 rounded-full h-2.5 mb-2">
					<div
						class="bg-blue-9 text-gray-1 dark:text-gray-12 h-2.5 rounded-full transition-all duration-200"
						style={{
							width: `${Math.max(0, Math.min(100, props.progressPercentage))}%`,
						}}
					/>
				</div>

				<p class="mt-2 text-xs text-gray-1 dark:text-gray-12">
					{typeof props.progressMessage === "string"
						? props.progressMessage
						: `${Math.floor(props.progressPercentage)}%`}
				</p>
			</div>
		</div>
	);
}

const IconButton = (props: ComponentProps<"button">) => {
	return (
		<button
			{...props}
			type="button"
			class={cx(
				"p-[0.325rem] bg-gray-1 dark:bg-gray-12 rounded-full text-[12px] shadow-[0px 2px 4px rgba(18, 22, 31, 0.12)]",
				props.class,
			)}
		/>
	);
};

const TooltipIconButton = (
	props: ComponentProps<"button"> & {
		tooltipText: string;
		tooltipPlacement: string;
	},
) => {
	const [isOpen, setIsOpen] = createSignal(false);

	return (
		<Tooltip
			placement={props.tooltipPlacement as "top" | "bottom" | "left" | "right"}
			openDelay={0}
			closeDelay={0}
			open={isOpen()}
			onOpenChange={setIsOpen}
			hoverableContent={false}
			floatingOptions={{
				offset: 10,
				flip: true,
				shift: true,
			}}
		>
			<Tooltip.Trigger as={IconButton} {...props}>
				{props.children}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Content
					class="py-1.5 px-2 font-medium"
					style={{
						"background-color": "rgba(255, 255, 255, 0.85)",
						color: "black",
						"border-radius": "8px",
						"font-size": "12px",
						"z-index": "15",
					}}
				>
					{props.tooltipText}
				</Tooltip.Content>
			</Tooltip.Portal>
		</Tooltip>
	);
};

function createFakeWindowBounds(
	ref: () => HTMLElement | undefined | null,
	key: Accessor<string>,
) {
	const bounds = createElementBounds(ref);

	const publish = (
		left: number,
		top: number,
		width: number,
		height: number,
	) => {
		commands.setFakeWindowBounds(key(), {
			position: { x: left, y: top },
			size: { width, height },
		});
	};

	createEffect(() => {
		publish(
			bounds.left ?? 0,
			bounds.top ?? 0,
			bounds.width ?? 0,
			bounds.height ?? 0,
		);
	});

	// A card that mounted while the overlay window was hidden measures as zero and never resizes
	// afterwards, which would leave the whole overlay click-through. Re-measure once it is shown.
	const remeasure = () => {
		if (document.hidden) return;
		requestAnimationFrame(() => {
			const element = ref();
			if (!element) return;
			const rect = element.getBoundingClientRect();
			publish(rect.left, rect.top, rect.width, rect.height);
		});
	};

	document.addEventListener("visibilitychange", remeasure);

	onCleanup(() => {
		document.removeEventListener("visibilitychange", remeasure);
		commands.removeFakeWindow(key());
	});
}

function createRecordingMutations(media: MediaEntry) {
	const type = media.type ?? "recording";
	const isRecording = type !== "screenshot";

	const recordingMeta = createQuery(() => ({
		queryKey: ["recordingMeta", media.path],
		queryFn: () => commands.getRecordingMeta(media.path, type),
	}));

	// just a wrapper of exportVideo to provide base settings
	const exportWithDefaultSettings = (
		onProgress: (progress: FramesRendered) => void,
	) =>
		exportVideo(
			media.path,
			{
				format: "Mp4",
				fps: FPS,
				resolution_base: OUTPUT_SIZE,
				compression: "Web",
				custom_bpp: null,
			},
			onProgress,
		);

	const copy = createMutation(() => ({
		mutationFn: async () => {
			setActionState({
				type: "copy",
				state: { type: "rendering", state: { type: "starting" } },
			});

			try {
				if (isRecording) {
					// First try to get existing rendered video
					const outputPath = await exportWithDefaultSettings(
						createRenderProgressCallback("copy", setActionState),
					);

					// Show quick progress animation for existing video
					setActionState(
						produce((s) => {
							if (
								s.type === "copy" &&
								s.state.type === "rendering" &&
								s.state.state.type === "rendering"
							)
								s.state.state.renderedFrames = s.state.state.totalFrames;
						}),
					);

					await commands.copyVideoToClipboard(outputPath);
				} else {
					// For screenshots, show quick progress animation
					setActionState({
						type: "copy",
						state: { type: "copying" },
					});
					await commands.copyScreenshotToClipboard(media.path);
				}

				setActionState({
					type: "copy",
					state: { type: "copied" },
				});
			} catch (error) {
				console.error("Error in copy media:", error);
				throw error;
			}
		},
		onSuccess() {
			setTimeout(() => {
				setActionState({ type: "idle" });
			}, 2000);
		},
	}));

	const save = createMutation(() => ({
		mutationFn: async () => {
			const meta = recordingMeta.data;
			if (!meta) {
				throw new Error("Recording metadata not available");
			}

			const defaultName = isRecording
				? "Shelf Recording"
				: media.path.split(".cap/")[1];
			const suggestedName = meta.pretty_name || defaultName;

			const fileType = isRecording ? "mp4" : "screenshot";
			const extension = isRecording ? ".mp4" : ".png";

			const fullFileName = suggestedName.endsWith(extension)
				? suggestedName
				: `${suggestedName}${extension}`;

			setActionState({
				type: "save",
				state: {
					type: "choosing-location",
					mediaType: isRecording ? "video" : "screenshot",
				},
			});

			if (isRecording) {
				const { promise } = createExportToFileTask(
					media.path,
					{
						format: "Mp4",
						fps: FPS,
						resolution_base: OUTPUT_SIZE,
						compression: "Web",
						custom_bpp: null,
					},
					fullFileName,
					fileType,
					createRenderProgressCallback("save", setActionState),
					() => {
						setActionState({
							type: "save",
							state: { type: "rendering", state: { type: "starting" } },
						});
					},
					() => {
						setActionState({ type: "save", state: { type: "saving" } });
					},
				);

				await promise;
			} else {
				const savePath = await commands.saveFileDialog(fullFileName, fileType);

				if (!savePath) {
					setActionState({ type: "idle" });
					return false;
				}

				setActionState({ type: "save", state: { type: "saving" } });

				await commands.copyFileToPath(media.path, savePath);
			}

			setActionState({ type: "save", state: { type: "saved" } });

			return true;
		},
		onSettled() {
			setTimeout(() => {
				setActionState({ type: "idle" });
			}, 2000);
		},
	}));

	const [actionState, setActionState] = createStore<ActionState>({
		type: "idle",
	});

	return { copy, save, actionState };
}

type ActionState =
	| { type: "idle" }
	| {
			type: "copy";
			state:
				| { type: "rendering"; state: RenderState }
				| { type: "copying" }
				| { type: "copied" };
	  }
	| {
			type: "save";
			state:
				| { type: "choosing-location"; mediaType: "video" | "screenshot" }
				| { type: "rendering"; state: RenderState }
				| { type: "saving" }
				| { type: "saved" };
	  };

function createRenderProgressCallback(
	actionType: Exclude<ActionState["type"], "idle">,
	setActionState: SetStoreFunction<ActionState>,
) {
	return (msg: FramesRendered) => {
		setActionState(
			produce((progressState) => {
				if (
					progressState.type !== actionType ||
					progressState.state.type !== "rendering"
				)
					return;

				if (progressState.state.state.type === "rendering")
					progressState.state.state = {
						type: "rendering",
						renderedFrames: msg.renderedCount,
						totalFrames: msg.totalFrames,
					};
			}),
		);
	};
}

function actionProgressPercentage(state: ActionState): number {
	function renderPercentage(state: RenderState) {
		if (state.type === "starting") return 0;
		else
			return Math.floor(
				Math.min((state.renderedFrames / state.totalFrames) * 100, 100),
			);
	}

	if (state.type === "idle") return 0;

	if (state.state.type === "rendering")
		return renderPercentage(state.state.state);

	switch (state.type) {
		case "copy": {
			return state.state.type === "copied" ? 100 : 50;
		}
		case "save": {
			if (state.state.type === "choosing-location") return 0;
			return state.state.type === "saved" ? 100 : 50;
		}
	}
}

type RenderState =
	| {
			type: "rendering";
			renderedFrames: number;
			totalFrames: number;
	  }
	| { type: "starting" };
