import * as dialog from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { commands, type RecordingAction, type RecordingMode } from "./tauri";

export function handleRecordingResult(result: Promise<RecordingAction>) {
	return result
		.then(async (result) => {
			if (result === "Started") return;
			await dialog.message(`Error: ${result}`, {
				title: "Error starting recording",
			});
		})
		.catch((err) =>
			dialog.message(err, {
				title: "Error starting recording",
				kind: "error",
			}),
		);
}

export async function openRecordingFolder(
	projectPath: string,
	mode: RecordingMode,
) {
	const path = projectPath.replace(/[/\\]+$/, "");

	const openedContent =
		mode === "instant" &&
		(await commands.openFilePath(`${path}/content`).then(
			() => true,
			() => false,
		));

	if (openedContent) return;

	await revealItemInDir(`${path}/`);
}
