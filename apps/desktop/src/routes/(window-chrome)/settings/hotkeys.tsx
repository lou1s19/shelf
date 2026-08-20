import { createEventListener } from "@solid-primitives/event-listener";
import { type as ostype } from "@tauri-apps/plugin-os";
import {
	batch,
	createMemo,
	createResource,
	createSignal,
	For,
	Index,
	Match,
	onMount,
	Show,
	Switch,
} from "solid-js";
import { createStore } from "solid-js/store";
import { hotkeysStore } from "~/store";

import {
	commands,
	type Hotkey,
	type HotkeyAction,
	type HotkeysStore,
} from "~/utils/tauri";
import {
	Section,
	SectionRows,
	SettingItem,
	SettingsPageContent,
} from "./Setting";

type ShortcutEntry = {
	id: HotkeyAction;
	label: string;
	hint?: string;
};

type ShortcutGroup = {
	title: string;
	description: string;
	entries: ShortcutEntry[];
};

/**
 * Single source for the order and the labels of every configurable shortcut.
 * An action that is not listed here cannot be set in the UI.
 */
const SHORTCUT_GROUPS: ShortcutGroup[] = [
	{
		title: "Recording",
		description:
			"System wide shortcuts for recording. The first one starts and stops with the same keys.",
		entries: [
			{
				id: "toggleRecording",
				label: "Start or stop recording",
				hint: "Starts in the selected mode and stops a recording that is already running.",
			},
			{ id: "togglePauseRecording", label: "Pause or resume recording" },
			{ id: "restartRecording", label: "Restart recording" },
			{ id: "cycleRecordingMode", label: "Switch recording mode" },
			{ id: "openRecordingPicker", label: "Open recording picker" },
		],
	},
	{
		title: "Screenshots",
		description: "Capture the screen without starting a recording.",
		entries: [
			{ id: "screenshotDisplay", label: "Screenshot current display" },
			{ id: "screenshotWindow", label: "Screenshot current window" },
			{ id: "screenshotArea", label: "Screenshot area" },
			{ id: "screenshotAreaOcr", label: "Area screenshot to text" },
		],
	},
];

const ACTION_LABELS = new Map<HotkeyAction, string>(
	SHORTCUT_GROUPS.flatMap((group) =>
		group.entries.map((entry) => [entry.id, entry.label] as const),
	),
);

/**
 * Actions that were replaced by "toggleRecording". A stored shortcut moves over
 * once, then the old entries are dropped so nothing stays registered invisibly.
 */
const RETIRED_TOGGLE_ACTIONS = [
	"stopRecording",
	"startStudioRecording",
	"startInstantRecording",
] as const;

type Hotkeys = { [K in HotkeyAction]?: Hotkey };

function migrateHotkeys(hotkeys: Hotkeys): Hotkeys {
	const next: Hotkeys = { ...hotkeys };

	if (!next.toggleRecording) {
		const source = RETIRED_TOGGLE_ACTIONS.find((action) => next[action]);
		if (source) next.toggleRecording = next[source];
	}

	for (const action of RETIRED_TOGGLE_ACTIONS) delete next[action];
	// "other" collects every name this version does not know.
	delete next.other;

	return next;
}

function bindingKey(binding: Hotkey) {
	return [
		binding.code,
		binding.meta,
		binding.ctrl,
		binding.alt,
		binding.shift,
	].join("+");
}

export default function () {
	const [store] = createResource(() => hotkeysStore.get());

	return (
		<Show when={store.state === "ready" && ([store()] as const)}>
			{(store) => <Inner initialStore={store()[0] ?? null} />}
		</Show>
	);
}

const MODIFIER_KEYS = new Set(["Meta", "Shift", "Control", "Alt"]);
function Inner(props: { initialStore: HotkeysStore | null }) {
	const [hotkeys, setHotkeys] = createStore<Hotkeys>(
		migrateHotkeys(props.initialStore?.hotkeys ?? {}),
	);

	// A shortcut must never be on disk without being registered with the system:
	// saving used to happen on every keystroke while registering waited for a
	// confirm click, so a shortcut looked set but did nothing until the next app
	// start. Both now happen together, and only after the system accepted it.
	// Swallowing the error here would produce the same trap from the other side:
	// the shortcut is registered and works, but it is gone after a restart. The
	// caller shows the failure instead.
	const persist = () =>
		hotkeysStore.set({ hotkeys: { ...hotkeys } as HotkeysStore["hotkeys"] });

	const [failures, setFailures] = createSignal<
		Partial<Record<HotkeyAction, string>>
	>({});

	// `previous` is passed in, not read from the store: both callers write the
	// new binding into the store before calling, so reading it here would find
	// the new value and the rollback would restore nothing.
	const apply = async (
		action: HotkeyAction,
		binding: Hotkey | null,
		previous: Hotkey | undefined,
	) => {
		const rollback = (message: string) => {
			// biome-ignore lint/style/noNonNullAssertion: store
			setHotkeys(action, previous ?? (undefined! as Hotkey));
			setFailures((prev) => ({ ...prev, [action]: message }));
		};

		try {
			await commands.setHotkey(action, binding);
		} catch (error) {
			rollback(`The system refused this shortcut: ${error}`);
			return;
		}

		setFailures((prev) => {
			const { [action]: _removed, ...rest } = prev;
			return rest;
		});

		try {
			await persist();
		} catch (error) {
			// Registered but not saved would work until the next start and then
			// be gone, so the registration goes back with it.
			await commands
				.setHotkey(action, previous ?? null)
				.catch((undoError) =>
					console.error("Failed to undo the shortcut:", undoError),
				);
			rollback(`The shortcut could not be saved: ${error}`);
		}
	};

	onMount(() => {
		const stored = props.initialStore?.hotkeys ?? {};
		const migrated = migrateHotkeys(stored);
		if (Object.keys(migrated).length !== Object.keys(stored).length)
			void persist();
	});

	const [listening, setListening] = createSignal<{
		action: HotkeyAction;
		prev?: Hotkey;
	}>();

	createEventListener(window, "keydown", (e) => {
		if (MODIFIER_KEYS.has(e.key)) return;

		const data = {
			code: e.code,
			ctrl: e.ctrlKey,
			shift: e.shiftKey,
			alt: e.altKey,
			meta: e.metaKey,
		};

		const l = listening();
		if (l) {
			e.preventDefault();

			const previous = hotkeys[l.action];
			batch(() => {
				setHotkeys(l.action, data);
				setListening();
			});
			void apply(l.action, data, previous);
		}
	});

	/**
	 * Two actions on the same keys both fire. That is not blocked, but every
	 * affected row says which other action it shares its shortcut with.
	 */
	const conflicts = createMemo(() => {
		const byBinding = new Map<string, HotkeyAction[]>();

		for (const action of ACTION_LABELS.keys()) {
			const binding = hotkeys[action];
			if (!binding) continue;

			const key = bindingKey(binding);
			byBinding.set(key, [...(byBinding.get(key) ?? []), action]);
		}

		const result = new Map<HotkeyAction, string>();
		for (const actions of byBinding.values()) {
			if (actions.length < 2) continue;

			for (const action of actions) {
				const others = actions
					.filter((other) => other !== action)
					.map((other) => ACTION_LABELS.get(other));
				result.set(action, `Same shortcut as ${others.join(", ")}.`);
			}
		}

		return result;
	});

	return (
		<div class="cap-settings-page flex flex-col h-full custom-scroll">
			<SettingsPageContent>
				<For each={SHORTCUT_GROUPS}>
					{(group) => (
						<Section title={group.title} description={group.description}>
							<SectionRows>
								<Index each={group.entries}>
									{(entry) => {
										const action = () => entry().id;

										createEventListener(window, "click", () => {
											if (listening()?.action !== action()) return;

											batch(() => {
												setHotkeys(action(), listening()?.prev);
												setListening();
											});
										});

										return (
											<SettingItem
												label={entry().label}
												description={
													failures()[action()] ??
													conflicts().get(action()) ??
													entry().hint
												}
											>
												<Switch>
													<Match when={listening()?.action === action()}>
														<div class="flex flex-row-reverse gap-2 justify-between items-center h-full text-sm rounded-lg w-fit">
															<Show
																when={hotkeys[action()]}
																fallback={
																	<p class="text-[13px] text-gray-11">
																		Set hotkeys...
																	</p>
																}
															>
																{(binding) => (
																	<HotkeyText binding={binding()} />
																)}
															</Show>
															<div class="flex flex-row items-center gap-0.5">
																<Show when={hotkeys[action()]}>
																	<button
																		class="w-fit"
																		type="button"
																		onClick={(e) => {
																			e.stopPropagation();
																			setListening();
																		}}
																	>
																		<IconCapCircleCheck class="transition-colors text-gray-12 hover:text-gray-10 size-5" />
																	</button>
																</Show>
																<button
																	type="button"
																	onClick={(e) => {
																		e.stopPropagation();
																		const previous = hotkeys[action()];
																		batch(() => {
																			setListening();
																			// biome-ignore lint/style/noNonNullAssertion: store
																			setHotkeys(action(), undefined!);
																		});
																		void apply(action(), null, previous);
																	}}
																>
																	<IconCapCircleX class="text-red-500 transition-colors hover:text-red-700 size-5" />
																</button>
															</div>
														</div>
													</Match>
													<Match when={listening()?.action !== action()}>
														<button
															type="button"
															class="text-sm bg-transparent rounded-lg"
															onClick={() => {
																// ensures that previously selected hotkey is cleared by letting the event propagate before listening to the new hotkey
																setTimeout(() => {
																	setListening({
																		action: action(),
																		prev: hotkeys[action()],
																	});
																}, 1);
															}}
														>
															<Show
																when={hotkeys[action()]}
																fallback={
																	<p
																		class="flex items-center text-[11px] uppercase transition-colors hover:bg-gray-6 hover:border-gray-7
                        py-3 px-2.5 h-5 bg-gray-4 border border-gray-5 rounded-lg text-gray-11 hover:text-gray-12"
																	>
																		None
																	</p>
																}
															>
																{(binding) => (
																	<HotkeyText binding={binding()} />
																)}
															</Show>
														</button>
													</Match>
												</Switch>
											</SettingItem>
										);
									}}
								</Index>
							</SectionRows>
						</Section>
					)}
				</For>
			</SettingsPageContent>
		</div>
	);
}

function HotkeyText(props: { binding: Hotkey }) {
	const os = ostype();
	const keys: string[] = [];

	if (os === "macos") {
		if (props.binding.meta) keys.push("⌘");
		if (props.binding.ctrl) keys.push("⌃");
		if (props.binding.alt) keys.push("⌥");
		if (props.binding.shift) keys.push("⇧");
	} else {
		if (props.binding.meta) keys.push("Win");
		if (props.binding.ctrl) keys.push("Ctrl");
		if (props.binding.alt) keys.push("Alt");
		if (props.binding.shift) keys.push("Shift");
	}

	const mainKey = props.binding.code.startsWith("Key")
		? props.binding.code[3]
		: props.binding.code;
	keys.push(mainKey);

	return (
		<div class="flex gap-1 items-center w-fit group">
			<For each={keys}>
				{(key) => (
					<kbd class="inline-flex justify-center w-fit text-xs items-center p-2 text-[13px] font-medium rounded-sm border size-6 text-gray-11 bg-gray-5 border-gray-6 group-hover:border-gray-8 transition-colors duration-200 group-hover:bg-gray-7">
						{key}
					</kbd>
				)}
			</For>
		</div>
	);
}
