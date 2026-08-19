import { createSignal, onCleanup, onMount } from "solid-js";
import { createTauriEventListener } from "~/utils/createEventListener";
import { commands, events } from "~/utils/tauri";

declare global {
	interface Window {
		__SHELF__?: { pinText?: string };
	}
}

const VISIBLE_MS = 4000;

export default function TextPin() {
	const [text, setText] = createSignal(window.__SHELF__?.pinText ?? "");

	let timer: ReturnType<typeof setTimeout> | undefined;
	let hovered = false;

	const hide = () => {
		commands
			.closeTextPinWindow()
			.catch((error) => console.error("Failed to hide copied text", error));
	};

	const schedule = () => {
		if (timer !== undefined) clearTimeout(timer);
		timer = setTimeout(hide, VISIBLE_MS);
	};

	onMount(() => {
		document.documentElement.setAttribute("data-transparent-window", "true");
		document.body.style.background = "transparent";
		schedule();
	});

	onCleanup(() => {
		if (timer !== undefined) clearTimeout(timer);
	});

	createTauriEventListener(events.textPinAdded, (payload) => {
		setText(payload.text);
		if (!hovered) schedule();
	});

	return (
		<div class="flex justify-center items-start w-screen h-screen bg-transparent">
			<div
				class="px-4 py-3 w-full rounded-2xl border shadow-lg backdrop-blur-xl"
				style={{
					// Fixed colours, not theme tokens: this panel sits under the notch
					// in both themes, and the token scale flips per window.
					background: "rgba(22, 24, 28, 0.94)",
					"border-color": "rgba(255, 255, 255, 0.12)",
					color: "#f5f6f8",
				}}
				onMouseEnter={() => {
					hovered = true;
					if (timer !== undefined) clearTimeout(timer);
				}}
				onMouseLeave={() => {
					hovered = false;
					schedule();
				}}
			>
				<p
					class="overflow-hidden text-sm leading-snug"
					style={{
						display: "-webkit-box",
						"-webkit-line-clamp": "2",
						"-webkit-box-orient": "vertical",
					}}
				>
					{text()}
				</p>
			</div>
		</div>
	);
}
