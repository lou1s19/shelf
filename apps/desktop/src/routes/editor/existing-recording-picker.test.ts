import { describe, expect, it } from "vitest";

import { getExistingRecordingPickerOptions } from "./existing-recording-picker";

describe("existing recording picker", () => {
	it("selects project directories on Windows", () => {
		expect(
			getExistingRecordingPickerOptions("windows", "C:\\Shelf\\recordings"),
		).toEqual({
			defaultPath: "C:\\Shelf\\recordings",
			directory: true,
			multiple: false,
		});
	});

	it.each(["macos", "linux"] as const)(
		"preserves the filtered file picker on %s",
		(platform) => {
			expect(
				getExistingRecordingPickerOptions(platform, "/Shelf/recordings"),
			).toEqual({
				defaultPath: "/Shelf/recordings",
				filters: [{ name: "Shelf Recording", extensions: ["cap"] }],
				multiple: false,
			});
		},
	);
});
