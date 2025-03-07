export interface Subtitle {
    start: number;
    end: number;
    text: string[];
}

export interface SubtitleCue {
    id: string;
    name: string;
    type: "original" | "translated";
    language: string;
    vttContent: string;
}