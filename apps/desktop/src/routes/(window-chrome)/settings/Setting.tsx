import { CheckMenuItem, Menu } from "@tauri-apps/api/menu";
import { cx } from "cva";
import type { JSX, ParentProps } from "solid-js";
import { For, onCleanup, onMount, Show } from "solid-js";
import { Toggle } from "~/components/Toggle";
import { events } from "~/utils/tauri";

export function SettingsPageContent(props: ParentProps<{ class?: string }>) {
	return (
		<div class={cx("px-6 py-6 space-y-7 max-w-[42rem]", props.class)}>
			{props.children}
		</div>
	);
}

export function Section(
	props: ParentProps<{
		title: string;
		description?: string | JSX.Element;
		right?: JSX.Element;
	}>,
) {
	return (
		<section class="space-y-2.5">
			<header class="flex justify-between items-end gap-3 px-1">
				<div class="flex flex-col gap-0.5 min-w-0">
					<h3 class="text-sm font-semibold tracking-tight text-gray-12">
						{props.title}
					</h3>
					<Show when={props.description}>
						<div class="text-xs leading-relaxed text-gray-10">
							{props.description}
						</div>
					</Show>
				</div>
				<Show when={props.right}>
					<div class="flex shrink-0 gap-2 items-center">{props.right}</div>
				</Show>
			</header>
			{props.children}
		</section>
	);
}

export function SectionCard(
	props: ParentProps<{ class?: string; padded?: boolean }>,
) {
	return (
		<div
			class={cx(
				"cap-settings-card overflow-hidden rounded-xl border border-gray-3 bg-gray-2",
				props.padded && "px-4 py-4",
				props.class,
			)}
		>
			{props.children}
		</div>
	);
}

export function SectionRows(props: ParentProps) {
	return (
		<SectionCard class="divide-y divide-gray-3">{props.children}</SectionCard>
	);
}

export function SettingItem(props: {
	id?: string;
	label: string;
	description?: string;
	children: JSX.Element;
}) {
	return (
		<div
			id={props.id}
			class="cap-setting-row flex flex-row gap-4 justify-between items-center px-4 py-3.5"
		>
			<div class="flex flex-col flex-1 min-w-0 gap-0.5">
				<p class="text-[13px] text-gray-12">{props.label}</p>
				<Show when={props.description}>
					<p class="text-xs leading-snug text-gray-10">{props.description}</p>
				</Show>
			</div>
			<div class="flex shrink-0 items-center">{props.children}</div>
		</div>
	);
}

export function ToggleSettingItem(props: {
	label: string;
	description?: string;
	value: boolean;
	onChange(v: boolean): void;
}) {
	return (
		<SettingItem {...props}>
			<Toggle
				size="sm"
				checked={props.value}
				onChange={(v) => props.onChange(v)}
			/>
		</SettingItem>
	);
}

/** Setting row with a native dropdown menu for its options. */
export function SelectSettingItem<T extends string | number>(props: {
	id?: string;
	label: string;
	description?: string;
	value: T;
	onChange: (value: T) => void;
	options: { text: string; value: T }[];
}) {
	return (
		<SettingItem
			id={props.id}
			label={props.label}
			description={props.description}
		>
			<button
				type="button"
				class="flex flex-row gap-1.5 text-xs items-center px-2.5 py-1.5 rounded-lg border transition-colors bg-gray-3 hover:bg-gray-4 text-gray-12 border-gray-4"
				onClick={async () => {
					const currentValue = props.value;
					const items = props.options.map((option) =>
						CheckMenuItem.new({
							text: option.text,
							checked: currentValue === option.value,
							action: () => props.onChange(option.value),
						}),
					);
					const menu = await Menu.new({ items: await Promise.all(items) });
					await menu.popup();
					await menu.close();
				}}
			>
				{(() => {
					const currentValue = props.value;
					const option = props.options.find(
						(opt) => opt.value === currentValue,
					);
					return option ? option.text : currentValue;
				})()}
				<IconCapChevronDown class="size-3.5 text-gray-10" />
			</button>
		</SettingItem>
	);
}

/** Small boxed switch used for a handful of mutually exclusive options. */
export function SegmentedControl<T extends string | number>(props: {
	value: T;
	onChange: (value: T) => void;
	options: { value: T; label: string }[];
	class?: string;
}) {
	return (
		<div
			class={cx(
				"inline-flex p-0.5 rounded-lg border border-gray-3 bg-gray-3",
				props.class,
			)}
		>
			<For each={props.options}>
				{(option) => {
					const isSelected = () => props.value === option.value;
					return (
						<button
							type="button"
							onClick={() => props.onChange(option.value)}
							class={cx(
								"px-3 py-1 text-xs font-medium rounded-md transition-[background-color,color,box-shadow]",
								isSelected()
									? "bg-gray-1 text-gray-12 shadow-sm"
									: "text-gray-10 hover:text-gray-12",
							)}
						>
							{option.label}
						</button>
					);
				}}
			</For>
		</div>
	);
}

export type SettingsView = "library" | "settings";

/**
 * Switch between the file list and the settings of a page that has both
 * (Recordings, Screenshots).
 */
export function SettingsViewSwitch(props: {
	value: SettingsView;
	onChange: (value: SettingsView) => void;
}) {
	return (
		<SegmentedControl
			value={props.value}
			onChange={props.onChange}
			options={[
				{ value: "library", label: "Library" },
				{ value: "settings", label: "Settings" },
			]}
		/>
	);
}

const SCROLL_TO_SECTION_KEY = "cap.settings.scrollToSection";

/**
 * Sections that other windows deep link to, and the settings page they live
 * on. Anything not listed here is expected on the General page.
 */
const SETTINGS_SECTION_PAGES: Record<string, string> = {
	"instant-quality": "recordings",
	"studio-quality": "recordings",
};

/**
 * Handles deep links to a single setting (see `requestScrollToSettingsSection`).
 * If the requested section lives on another page, the request is stored and the
 * window navigates there, where this hook picks it up again on mount.
 */
export function createSettingsSectionScroll(options: {
	page: string;
	container: () => HTMLElement | undefined;
	navigate: (page: string) => void;
	onSection?: (section: string) => void;
}) {
	const handle = (section: string) => {
		const targetPage = SETTINGS_SECTION_PAGES[section] ?? "general";
		if (targetPage !== options.page) {
			try {
				localStorage.setItem(SCROLL_TO_SECTION_KEY, section);
			} catch {}
			options.navigate(targetPage);
			return;
		}

		try {
			localStorage.removeItem(SCROLL_TO_SECTION_KEY);
		} catch {}
		options.onSection?.(section);

		const attempt = (remaining: number) => {
			const target = document.getElementById(`settings-section-${section}`);
			const container = options.container();
			if (!target || !container) {
				if (remaining > 0) {
					window.setTimeout(() => attempt(remaining - 1), 50);
				}
				return;
			}
			const containerRect = container.getBoundingClientRect();
			const targetRect = target.getBoundingClientRect();
			const offset =
				targetRect.top - containerRect.top + container.scrollTop - 8;
			container.scrollTo({ top: offset, behavior: "smooth" });
			target.classList.add("settings-section-pulse");
			window.setTimeout(() => {
				target.classList.remove("settings-section-pulse");
			}, 1600);
		};
		attempt(10);
	};

	onMount(() => {
		let pending: string | null = null;
		try {
			pending = localStorage.getItem(SCROLL_TO_SECTION_KEY);
		} catch {}
		if (pending) handle(pending);

		const unlisten = events.requestScrollToSettingsSection.listen((event) =>
			handle(event.payload.section),
		);
		onCleanup(() => {
			unlisten.then((cb) => cb()).catch(() => {});
		});
	});
}
