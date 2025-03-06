import type {Subtitle} from "./video-subtitle-viewer-types";

export const parseVTT = (vttContent: string): Subtitle[] => {
    const lines = vttContent.split("\n");
    const subtitles: Subtitle[] = [];
    let currentSubtitle: Subtitle | null = null;

    for (const line of lines) {
        if (line.includes("-->")) {
            const [startTime, endTime] = line.split("-->").map((time) => {
                const [hours, minutes, seconds] = time.trim().split(/[:.]/).map(Number.parseFloat);
                return hours * 3600 + minutes * 60 + seconds + (seconds % 1);
            });

            currentSubtitle = { start: startTime, end: endTime, text: [] };
        } else if (line.trim() !== "" && line.trim() !== "WEBVTT" && currentSubtitle) {
            currentSubtitle.text.push(line.trim());

            if (!lines[lines.indexOf(line) + 1]?.includes("-->") &&
                lines[lines.indexOf(line) + 1]?.trim() === "") {
                subtitles.push(currentSubtitle);
                currentSubtitle = null;
            }
        }
    }
    return subtitles;
};

export const formatTime = (seconds: number): string => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    const millis = Math.floor((seconds % 1) * 1000);

    return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
};

export const convertSRTtoVTT = (srt: string): string => {
    return srt
        .replace(/(\d{2}:\d{2}:\d{2}),(\d{3})/g, "$1.$2")
        .replace(/{.*?}/g, "")
        .replace(/WEBVTT\n\n/, "");
};
